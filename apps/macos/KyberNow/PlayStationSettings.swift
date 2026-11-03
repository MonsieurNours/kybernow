// Réglages d'une console PlayStation (docs/link-ui.md, « Consoles
// PlayStation », « Réglages de la console ») : présentés comme ceux d'un
// ordinateur (en tête son nom, à gauche les rubriques Image et Commandes, à
// droite leurs cartes en verre). Chaque changement est enregistré tout de
// suite et vaut pour la partie suivante ; le clavier et la souris,
// l'agrandissement et les intensités de la manette changent aussi tout de
// suite pendant une partie.

import AppKit
import KyberKit
import SwiftUI

/// Les rubriques des réglages d'une console, dans cet ordre.
enum ConsoleSection: String, CaseIterable, Identifiable {
    case picture, controls

    var id: String { rawValue }

    var title: String {
        switch self {
        case .picture: return String(localized: "Image")
        case .controls: return String(localized: "Commandes")
        }
    }

    var icon: String {
        switch self {
        case .picture: return "image"
        case .controls: return "game-controller"
        }
    }
}

extension PSLayout {
    /// La disposition par défaut du cœur.
    static let standard: PSLayout = (try? JSONDecoder().decode(PSLayout.self, from: Data(PlayStation.defaultLayout.utf8))) ?? PSLayout()
}

struct ConsoleSettingsView: View {
    @EnvironmentObject var app: AppModel
    @ObservedObject private var store = AppModel.shared.playstation
    let consoleID: UUID

    var body: some View {
        if let console = store.console(consoleID) {
            ConsoleSettingsContent(console: console, store: store)
        } else {
            Color.clear.onAppear { app.route = .home }
        }
    }
}

private struct ConsoleSettingsContent: View {
    @EnvironmentObject var app: AppModel
    let console: PlayStationConsole
    @ObservedObject var store: PlayStationStore
    @StateObject private var catcher = KeyCatcher()

    private var s: PSSettings { store.settings(console) }
    private var l: PSLayout { store.layout(console) }
    /// La PS4 n'envoie que du H.264.
    private var ps4: Bool { !console.hasHDR }

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
        ZStack {
            Theme.background.ignoresSafeArea()
            Signature(seed: console.hostID.isEmpty ? console.name : console.hostID)
                .blur(radius: 40)
                .opacity(0.55)
                .ignoresSafeArea()
            LinearGradient(colors: [Theme.background.opacity(0.35), Theme.background.opacity(0.85)], startPoint: .top, endPoint: .bottom)
                .ignoresSafeArea()
            VStack(spacing: 0) {
                topBar
                header
                    .padding(.horizontal, 48)
                    .padding(.top, 18)
                    .padding(.bottom, 22)
                HStack(alignment: .top, spacing: 28) {
                    sidebar
                        .frame(width: 236)
                    ScrollView {
                        detail
                            .padding(.bottom, 32)
                            .frame(maxWidth: .infinity, alignment: .topLeading)
                    }
                    .scrollIndicators(.automatic)
                }
                .padding(.horizontal, 48)
            }
        }
        .preferredColorScheme(.dark)
        .frame(minWidth: 1100, minHeight: 760)
        .onDisappear { catcher.stop() }
    }

    // MARK: - En-tête

    private var topBar: some View {
        ZStack {
            HStack(spacing: 8) {
                Mark(size: 16)
                Wordmark(size: 13)
            }
            HStack {
                Button { app.route = .home } label: {
                    HStack(spacing: 8) {
                        Icon("arrow-left", size: 14, bold: true)
                        Text("Retour")
                    }
                }
                .buttonStyle(GhostButton(height: 30))
                .keyboardShortcut(.cancelAction)
                Spacer()
            }
            .padding(.leading, 84)
        }
        .frame(height: 52)
    }

    private var header: some View {
        HStack(alignment: .bottom, spacing: 24) {
            Text(LinkText.settingsTitle(console.name))
                .font(Theme.display(40, .bold)).tracking(-40 * 0.035)
                .foregroundStyle(Theme.text)
                .lineLimit(1).minimumScaleFactor(0.6)
                .accessibilityAddTraits(.isHeader)
            Spacer(minLength: 16)
            HStack(spacing: 12) {
                Theme.eyebrow(console.model, color: Theme.muted)
                HStack(spacing: 8) {
                    Icon("sliders-horizontal", size: 16, bold: true)
                    Text(store.isCustom(console) ? PSText.custom : PSText.auto)
                }
                .font(Theme.sans(14, .semibold))
                .foregroundStyle(Theme.text2)
                .padding(.horizontal, 14)
                .frame(height: 36)
                .glass(10)
            }
        }
    }

    // MARK: - Rubriques

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(ConsoleSection.allCases) { section in
                let on = app.consoleSection == section
                Button { withAnimation(Theme.easeOut) { app.consoleSection = section } } label: {
                    HStack(spacing: 12) {
                        Icon(section.icon, size: 18)
                            .foregroundStyle(on ? Theme.accent : Theme.muted)
                            .frame(width: 32, height: 32)
                            .background(on ? Theme.accentSoft : Color.clear, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                        VStack(alignment: .leading, spacing: 2) {
                            Text(section.title).font(Theme.sans(14.5, .semibold)).foregroundStyle(on ? Theme.text : Theme.text2)
                            Text(verbatim: subtitle(section)).font(Theme.sans(12)).foregroundStyle(Theme.faint).lineLimit(1)
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(.horizontal, 10).padding(.vertical, 8)
                    .background {
                        if on {
                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                .fill(Color.white.opacity(0.07))
                                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Color.white.opacity(0.09)))
                        }
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
        .padding(8)
        .glass(Theme.radiusCard, tint: 0.5)
    }

    /// Ce que contient chaque rubrique, en une ligne.
    private func subtitle(_ section: ConsoleSection) -> String {
        switch section {
        case .picture: return "\(PSSettings.resolutionLabel(s.resolution)) · \(PSText.fps(s.fps))"
        case .controls: return l == .standard ? PSText.defaultLayout : PSText.custom
        }
    }

    @ViewBuilder private var detail: some View {
        Group {
            switch app.consoleSection {
            case .picture: pictureSection
            case .controls: controlsSection
            }
        }
        .id(app.consoleSection)
        .transition(.opacity)
    }

    /// Deux colonnes de cartes quand la fenêtre est large, une sinon.
    private func columns<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 400), spacing: 18, alignment: .top)], alignment: .leading, spacing: 18) {
            content()
        }
    }

    private func label(_ text: String) -> some View {
        Text(text).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
    }

    private func help(_ text: String) -> some View {
        Text(text).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
    }

    private func toggleRow(_ title: String, _ note: String? = nil, isOn: Binding<Bool>) -> some View {
        HStack(alignment: note == nil ? .center : .top, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                if let note { help(note) }
            }
            Spacer(minLength: 0)
            Switch(isOn: isOn, label: title)
        }
    }

    // MARK: - Image

    private var pictureSection: some View {
        columns {
            pictureCard
            bitrateCard
        }
    }

    private var pictureCard: some View {
        SettingsCard(icon: "image", title: ConsoleSection.picture.title) {
            VStack(alignment: .leading, spacing: 8) {
                label(PSText.resolution)
                Segmented(options: PSSettings.resolutions.map { ($0, PSSettings.resolutionLabel($0)) },
                          selection: Binding(get: { s.resolution }, set: { r in if let r { update { $0.resolution = r } } }),
                          height: 30, fill: true)
                if s.resolution == "auto" { help(PSText.resolutionAutoHelp) }
            }
            VStack(alignment: .leading, spacing: 8) {
                label(PSText.frameRate)
                Segmented(options: [(60, PSText.fps(60)), (30, PSText.fps(30))],
                          selection: Binding(get: { s.fps == 30 ? 30 : 60 }, set: { f in if let f { update { $0.fps = f } } }),
                          height: 30, fill: true)
            }
            VStack(alignment: .leading, spacing: 8) {
                VStack(alignment: .leading, spacing: 8) {
                    label(PSText.format)
                    Segmented(options: PSSettings.codecs.map { ($0, $0 == "h264" ? "H.264" : $0 == "h265" ? "H.265" : PSText.auto) },
                              selection: Binding(get: { ps4 ? "h264" : s.codec }, set: { c in if let c, !ps4 { update { $0.codec = c } } }),
                              height: 30, fill: true)
                }
                .disabled(ps4)
                .opacity(ps4 ? 0.45 : 1)
                if ps4 {
                    help(PSText.formatPS4Help)
                } else if s.codec == "auto" {
                    help(PSText.formatAutoHelp)
                } else if s.codec == "h264" {
                    help(PSText.formatH264Help)
                }
            }
            toggleRow(PSText.fullScreen, isOn: Binding(get: { s.fullscreen }, set: { v in update { $0.fullscreen = v } }))
            toggleRow(PSText.upscale, PSText.upscaleHelp, isOn: Binding(get: { s.upscale }, set: { v in update { $0.upscale = v } }))
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    label(PSText.sharpness)
                    Spacer()
                    Text(PSText.percent(s.sharpness)).font(Theme.mono(13)).foregroundStyle(Theme.text)
                }
                Slider(value: Binding(get: { Double(s.sharpness) }, set: { v in
                    let n = min(max(Int((v / 5).rounded()) * 5, 0), 100)
                    update { $0.sharpness = n }
                }), in: 0...100, step: 5)
                .tint(Theme.accent)
                .accessibilityLabel(PSText.sharpness)
                .accessibilityValue(PSText.percent(s.sharpness))
            }
            .disabled(!s.upscale)
            .opacity(s.upscale ? 1 : 0.45)
        }
    }

    private var bitrateCard: some View {
        let auto = s.bitrateKbps == nil
        let mbps = (s.bitrateKbps ?? PSSettings.autoBitrate(s.resolution)) / 1000
        let range = PSSettings.bitrateRange
        return SettingsCard(icon: "lightning", title: PSText.bitrate) {
            toggleRow(PSText.auto, PSText.bitrateAutoHelp, isOn: Binding(get: { auto }, set: { on in
                update { $0.bitrateKbps = on ? nil : PSSettings.autoBitrate($0.resolution) }
            }))
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    label(PSText.bitrate)
                    Spacer()
                    Text(PSText.mbps(mbps)).font(Theme.mono(13)).foregroundStyle(Theme.text)
                }
                Slider(value: Binding(get: { Double(mbps) }, set: { v in
                    let n = min(max(Int(v.rounded()), range.lowerBound), range.upperBound)
                    update { $0.bitrateKbps = n * 1000 }
                }), in: Double(range.lowerBound)...Double(range.upperBound), step: 1)
                .tint(Theme.accent)
                .accessibilityLabel(PSText.bitrate)
                .accessibilityValue(PSText.mbps(mbps))
                HStack {
                    Text(verbatim: "\(range.lowerBound)").font(Theme.mono(11)).foregroundStyle(Theme.faint)
                    Spacer()
                    Text(verbatim: "\(range.upperBound)").font(Theme.mono(11)).foregroundStyle(Theme.faint)
                }
            }
            .disabled(auto)
            .opacity(auto ? 0.45 : 1)
        }
    }

    // MARK: - Commandes

    private var controlsSection: some View {
        columns {
            keysCard
            mouseCard
            controllerCard
        }
    }

    /// Intensités de la DualSense : vibrations et gâchettes adaptatives, de
    /// 0 à 100 % par pas de 10.
    private var controllerCard: some View {
        SettingsCard(icon: "game-controller", title: PSText.controller) {
            intensityRow(PSText.rumble, value: s.rumble) { v in update { $0.rumble = v } }
            intensityRow(PSText.adaptiveTriggers, value: s.triggers) { v in update { $0.triggers = v } }
            help(PSText.controllerHelp)
        }
    }

    private func intensityRow(_ title: String, value: Int, set: @escaping (Int) -> Void) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                label(title)
                Spacer()
                Text(PSText.percent(value)).font(Theme.mono(13)).foregroundStyle(Theme.text)
            }
            Slider(value: Binding(get: { Double(value) }, set: { v in
                let n = min(max(Int((v / 10).rounded()) * 10, 0), 100)
                if n != value { set(n) }
            }), in: 0...100, step: 10)
            .tint(Theme.accent)
            .accessibilityLabel(title)
            .accessibilityValue(PSText.percent(value))
        }
    }

    private var keysCard: some View {
        SettingsCard(icon: "keyboard", title: ConsoleSection.controls.title) {
            help(PSText.controlsHelp)
            VStack(spacing: 0) {
                ForEach(PSLayout.actions, id: \.self) { action in
                    actionRow(action)
                    if action != PSLayout.actions.last {
                        Rectangle().fill(Theme.line).frame(height: 1)
                    }
                }
            }
            Button { catcher.stop(); store.resetLayout(for: console) } label: {
                HStack(spacing: 8) { Icon("arrow-counter-clockwise", size: 15, bold: true); Text(PSText.defaultLayout) }
            }
            .buttonStyle(GhostButton(height: 36))
            .disabled(l == .standard)
        }
    }

    /// Une action de la manette : son nom, ses touches, « + ».
    private func actionRow(_ action: String) -> some View {
        let inputs = l.inputs(action)
        let adding = catcher.waiting == KeyCatcher.Wait(action: action, replacing: nil)
        return HStack(spacing: 8) {
            Text(PSText.action(action))
                .font(Theme.sans(13.5)).foregroundStyle(Theme.text2)
                .lineLimit(1).minimumScaleFactor(0.8)
            Spacer(minLength: 8)
            if inputs.isEmpty, !adding {
                Text(PSText.none).font(Theme.sans(12.5)).foregroundStyle(Theme.faint)
            }
            ForEach(inputs, id: \.self) { input in
                if catcher.waiting == KeyCatcher.Wait(action: action, replacing: input) {
                    waitingChip
                } else {
                    Button { listen(action, replacing: input) } label: { chip(PSText.input(input)) }
                        .buttonStyle(.plain)
                }
            }
            if adding {
                waitingChip
            } else {
                Button { listen(action, replacing: nil) } label: {
                    Icon("plus", size: 13, bold: true)
                        .foregroundStyle(Theme.muted)
                        .frame(width: 26, height: 26)
                        .background(Color.white.opacity(0.04), in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(Theme.line2))
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Text(verbatim: "+ \(PSText.action(action))"))
            }
        }
        .padding(.vertical, 6)
    }

    private func chip(_ text: String) -> some View {
        Text(verbatim: text)
            .font(Theme.sans(12.5, .semibold))
            .foregroundStyle(Theme.text)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 10)
            .frame(minWidth: 26, minHeight: 26)
            .background(Theme.well, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(Theme.line2))
            .contentShape(Rectangle())
    }

    private var waitingChip: some View {
        Text(PSText.pressKey)
            .font(Theme.sans(12.5, .semibold))
            .foregroundStyle(Theme.accent)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 10)
            .frame(minHeight: 26)
            .background(Theme.accentSoft, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(Theme.accentLine))
    }

    /// Attend la touche ou le bouton de la souris de cette action.
    private func listen(_ action: String, replacing: PSInput?) {
        let store = store, console = console
        catcher.start(KeyCatcher.Wait(action: action, replacing: replacing)) { wait, outcome in
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

    private var mouseCard: some View {
        let curve = PSLayout.curves.enumerated().min { abs($0.element - l.curve) < abs($1.element - l.curve) }?.offset ?? 2
        return SettingsCard(icon: "cursor", title: String(localized: "Souris")) {
            toggleRow(PSText.mouseStick, isOn: Binding(get: { l.mouseStick }, set: { v in updateLayout { $0.mouseStick = v } }))
            VStack(alignment: .leading, spacing: 16) {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        label(PSText.sensitivity)
                        Spacer()
                        Text(verbatim: l.sensitivity.formatted(.number.precision(.fractionLength(1)))).font(Theme.mono(13)).foregroundStyle(Theme.text)
                    }
                    Slider(value: Binding(get: { l.sensitivity }, set: { v in updateLayout { $0.sensitivity = (v * 10).rounded() / 10 } }), in: 0.2...5, step: 0.1)
                        .tint(Theme.accent)
                        .accessibilityLabel(PSText.sensitivity)
                }
                VStack(alignment: .leading, spacing: 8) {
                    label(PSText.startingResponse)
                    Segmented(options: PSText.curves.enumerated().map { ($0.offset, $0.element) },
                              selection: Binding(get: { curve }, set: { i in if let i { updateLayout { $0.curve = PSLayout.curves[i] } } }),
                              height: 30, fill: true)
                }
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        label(PSText.minimumTilt)
                        Spacer()
                        Text(verbatim: l.deadzone.formatted(.percent.precision(.fractionLength(0)))).font(Theme.mono(13)).foregroundStyle(Theme.text)
                    }
                    Slider(value: Binding(get: { l.deadzone }, set: { v in updateLayout { $0.deadzone = (v * 100).rounded() / 100 } }), in: 0...0.5, step: 0.01)
                        .tint(Theme.accent)
                        .accessibilityLabel(PSText.minimumTilt)
                    help(PSText.minimumTiltHelp)
                }
                toggleRow(PSText.invertY, isOn: Binding(get: { l.invertY }, set: { v in updateLayout { $0.invertY = v } }))
            }
            .disabled(!l.mouseStick)
            .opacity(l.mouseStick ? 1 : 0.45)
        }
    }
}

/// Attend la touche ou le bouton de la souris d'une action : Échap annule,
/// Effacement retire la touche qu'on changeait ; le reste est pris (sans
/// atteindre l'app, pas même le raccourci « Retour »).
@MainActor
final class KeyCatcher: ObservableObject {
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
    private var monitor: Any?
    private var done: ((Wait, Outcome) -> Void)?

    func start(_ wait: Wait, done: @escaping (Wait, Outcome) -> Void) {
        stop()
        waiting = wait
        self.done = done
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged, .leftMouseDown, .rightMouseDown, .otherMouseDown]) { [weak self] event in
            MainActor.assumeIsolated { self?.handle(event) ?? event }
        }
    }

    func stop() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        waiting = nil
        done = nil
    }

    private func handle(_ event: NSEvent) -> NSEvent? {
        guard let wait = waiting else { return event }
        let outcome: Outcome
        switch event.type {
        case .keyDown:
            if event.isARepeat { return nil }
            switch event.keyCode {
            case 53: outcome = .cancel  // Échap
            case 51: outcome = .remove  // Effacement
            default:
                guard let usage = Keymap.usage(keyCode: event.keyCode) else { return nil }
                outcome = .input(.key(usage))
            }
        case .flagsChanged:
            guard Keymap.modifierPressed(event) == true, let usage = Keymap.usage(keyCode: event.keyCode) else { return nil }
            outcome = .input(.key(usage))
        case .leftMouseDown: outcome = .input(.mouse(0))
        case .rightMouseDown: outcome = .input(.mouse(1))
        case .otherMouseDown: outcome = .input(.mouse(UInt8(min(max(event.buttonNumber, 2), 4))))
        default: return event
        }
        let finish = done
        stop()
        finish?(wait, outcome)
        return nil
    }
}
