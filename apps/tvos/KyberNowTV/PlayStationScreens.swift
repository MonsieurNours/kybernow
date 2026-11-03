// Consoles PlayStation sur l'Apple TV : la partie à l'écran et son menu, et
// les « Réglages de {nom} » d'une console (textes : docs/link-ui.md,
// « Consoles PlayStation »).
//
// La partie : l'image seule dans le même GCEventViewController qu'une
// session d'ordinateur (`SessionStage`) ; par-dessus, seulement quand ils
// servent, « Connexion à la console… », le contenu protégé, la fin, et le
// menu (la « barre de la partie » des autres apps, en tuiles comme le menu
// d'une session d'ordinateur) : Reprendre, Manettes, Son, Réglages de {nom},
// Mettre la console en veille, Se déconnecter ; les statistiques en haut.
// Pas de clavier ni de souris : la partie se joue à la manette.

import KyberKit
import SwiftUI

struct ConsoleSessionView: View {
    @ObservedObject var session: PlayStationSession
    @EnvironmentObject private var pads: GamepadBridge

    private var streaming: Bool { session.phase == .streaming }

    var body: some View {
        ZStack {
            Color.black
            SessionStage(view: session.streamView, gameOnly: streaming && !session.menuOpen,
                         scale: session.menuOpen ? 0.94 : 1, corner: session.menuOpen ? 28 : 0)
                .frame(width: 1920, height: 1080)
                .overlay {
                    // Le voile seulement le temps du menu : une couche laissée
                    // au-dessus de l'image obligerait tvOS à la composer.
                    if session.menuOpen {
                        ZStack {
                            Rectangle().fill(.ultraThinMaterial).opacity(0.35)
                            Color.black.opacity(0.58)
                        }
                        .allowsHitTesting(false)
                        .transition(.opacity)
                    }
                }

            switch session.phase {
            case .connecting:
                ZStack {
                    Signature(seed: session.console.name)
                    Color.black.opacity(0.35)
                    VStack(spacing: 28) {
                        Text(verbatim: session.console.name).font(Theme.display(96, 680)).tracking(-96 * 0.045).foregroundStyle(Theme.text)
                        HStack(spacing: 16) {
                            ProgressView()
                            Text(PSText.connecting).font(Theme.sans(28)).foregroundStyle(Theme.text2)
                        }
                        HStack(spacing: 12) {
                            Glyph(kind: .remote, role: .back, size: 26)
                            Text(StationText.cancel).font(Theme.sans(22)).foregroundStyle(Theme.muted)
                        }
                        .padding(.top, 30)
                    }
                }
                .transition(.opacity)
            case .streaming:
                if session.cantDisplay, !session.menuOpen {
                    MenuHint(text: PSText.cantDisplay, glyph: false)
                }
            case let .ended(text):
                ConsoleEnd(session: session, text: text)
                    .transition(.opacity)
            }

            if session.menuHint, !session.menuOpen {
                MenuHint(text: PSTVText.menuHint)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, 48)
                    .transition(.opacity)
            }
            if let note = pads.notice, streaming, !session.menuOpen {
                MenuHint(text: note, glyph: false)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, 124)
                    .transition(.opacity)
            }
            if session.menuOpen {
                ConsoleMenu(session: session)
                    .transition(.opacity)
            }
        }
        .frame(width: 1920, height: 1080)
        .ignoresSafeArea()
        .animation(Theme.easeOut, value: session.phase)
        .onExitCommand {
            // Retour quand l'app a la main : ferme le menu, ou annule la connexion.
            if session.settingsOpen {
                session.settingsOpen = false
            } else if session.menuOpen {
                session.closeMenu()
            } else if session.phase != .streaming {
                session.close()
            }
        }
    }
}

/// La fin d'une partie qui n'a pas été voulue ici : la raison, et « Fermer ».
private struct ConsoleEnd: View {
    @ObservedObject var session: PlayStationSession
    let text: String?
    @FocusState private var focused: Bool

    var body: some View {
        ZStack {
            Signature(seed: session.console.name)
            Color.black.opacity(0.4)
            VStack(alignment: .leading, spacing: 26) {
                Text(verbatim: session.console.name).font(Theme.display(72, 660)).tracking(-72 * 0.04).foregroundStyle(Theme.text)
                if let text {
                    Text(text).font(Theme.sans(28)).foregroundStyle(Theme.text2).frame(maxWidth: 1000, alignment: .leading)
                }
                Button(String(localized: "Fermer")) { session.close() }
                    .buttonStyle(PrimaryButton(height: 76, fontSize: 26))
                    .focused($focused)
                    .padding(.top, 16)
            }
            .padding(.leading, 160)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .onAppear { focused = true }
    }
}

// MARK: - Menu de la partie

private struct ConsoleMenu: View {
    @ObservedObject var session: PlayStationSession
    @EnvironmentObject private var pads: GamepadBridge
    @EnvironmentObject private var playstation: PlayStationStore
    @EnvironmentObject private var app: AppModel
    @State private var showPads = false
    @FocusState private var focus: Item?

    enum Item: Hashable { case resume, pads, sound, settings, rest, quit }

    private let tileCount = 6
    private let gap: CGFloat = 24
    private let resumeScale: CGFloat = 1.3
    private var tileWidth: CGFloat { (1920 - 192 - CGFloat(tileCount - 1) * gap) / (CGFloat(tileCount) + resumeScale - 1) }

    var body: some View {
        ZStack(alignment: .topLeading) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 18) {
                        Theme.eyebrow(String(localized: "En session depuis \(minutesLabel)"))
                        if session.path.viaRelay { relayChip }
                    }
                    Text(verbatim: session.console.name).font(Theme.display(60, 660)).tracking(-60 * 0.035).foregroundStyle(Theme.text)
                        .lineLimit(1).minimumScaleFactor(0.5)
                    Text(verbatim: session.console.model).font(Theme.sans(22)).foregroundStyle(Theme.muted)
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 10) {
                    Theme.eyebrow(String(localized: "Statistiques"))
                    stats
                }
            }
            .frame(width: 1920 - 192)
            .padding(.leading, 96).padding(.top, 56)

            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: gap) {
                    Button { session.closeMenu() } label: {
                        VStack(alignment: .leading) {
                            Icon("play", size: 44, bold: true)
                            Spacer()
                            VStack(alignment: .leading, spacing: 6) {
                                Text("Reprendre").font(Theme.sans(32, .bold)).lineLimit(1).minimumScaleFactor(0.8)
                                Text("La manette repart vers le jeu").font(Theme.sans(19)).foregroundStyle(.white.opacity(0.8))
                            }
                        }
                        .padding(28)
                        .frame(width: tileWidth * resumeScale, height: 230, alignment: .leading)
                    }
                    .buttonStyle(ResumeTileStyle())
                    .focused($focus, equals: .resume)
                    tile(.pads, icon: "game-controller", title: String(localized: "Manettes"), detail: LinkText.controllers(pads.pads.count)) { showPads = true }
                    tile(.sound, icon: session.muted ? "speaker-slash" : "speaker-high", title: session.muted ? PSText.unmute : PSText.mute,
                         detail: AudioRoute.current.name) { session.toggleMute() }
                    tile(.settings, icon: "sliders-horizontal", title: LinkText.settingsTitle(session.console.name),
                         detail: playstation.isCustom(session.console) ? PSText.custom : PSText.auto) { session.settingsOpen = true }
                    tile(.rest, icon: "power", title: PSText.rest, detail: session.console.model) { session.goToRest() }
                    tile(.quit, icon: "x", iconColor: Theme.danger, title: String(localized: "Se déconnecter"), detail: PSTVText.quitDetail) { session.close() }
                }
                .focusSection()
                HStack(spacing: 12) {
                    Glyph(kind: .remote, role: .back, size: 28)
                    Text(PSTVText.menuClose)
                    Spacer()
                    Text("Pendant le menu, les manettes pilotent KyberNow Link ; le jeu ne reçoit rien.")
                }
                .font(Theme.sans(19))
                .foregroundStyle(Theme.muted)
            }
            .frame(width: 1920 - 192)
            .padding(.leading, 96).padding(.top, 1080 - 72 - 230 - 22 - 28)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .defaultFocus($focus, .resume)
        .onAppear {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { focus = .resume }
        }
        .fullScreenCover(isPresented: $showPads) {
            GamepadsView(inSession: true) { showPads = false }
                .environmentObject(pads)
        }
        .fullScreenCover(isPresented: $session.settingsOpen) {
            ConsoleSettingsView(consoleID: session.console.id) { session.settingsOpen = false }
                .environmentObject(playstation)
                .environmentObject(app)
        }
    }

    private var minutesLabel: String {
        let m = session.sessionMinutes
        let mm = String(format: "%02d", m % 60)
        return m < 60 ? String(localized: "\(m) min") : String(localized: "\(m / 60) h \(mm)")
    }

    /// Les mesures de la partie (link-ui, « Statistiques » de la barre).
    private var stats: some View {
        VStack(alignment: .trailing, spacing: 8) {
            if let s = session.stats, s.width > 0 {
                Text(verbatim: PSText.statsAsked(s, fps: session.requestedFps)).foregroundStyle(Theme.text)
                Text(verbatim: PSText.statsReceived(s))
            } else {
                Text(verbatim: "—")
            }
            Text(verbatim: PSText.statsLosses(lost: session.lostTotal, repaired: session.repairedTotal))
            if let line = upscaleLine { Text(verbatim: line) }
            Text(verbatim: session.path.label)
        }
        .font(Theme.mono(18))
        .foregroundStyle(Theme.text2)
        .padding(.horizontal, 30).padding(.vertical, 18)
        .glass(20, tint: 0.55)
    }

    private var upscaleLine: String? {
        guard let r = session.upscale else { return nil }
        switch r.state {
        case let .active(w, h): return PSTVText.upscaled(w, h, ms: r.gpuMs)
        case .tooSlow: return PSTVText.upscaleOff
        case .notNeeded, .unavailable: return nil
        }
    }

    private var relayChip: some View {
        HStack(spacing: 10) {
            Icon("globe-hemisphere-west", size: 22, bold: true).foregroundStyle(Theme.accentLight)
            Text(LinkText.viaRelay).font(Theme.sans(20, .semibold)).foregroundStyle(Theme.text)
        }
        .padding(.horizontal, 18)
        .frame(height: 40)
        .glass(20, tint: 0.55)
    }

    private func tile(_ item: Item, icon: String, iconColor: Color = Theme.accent, title: String, detail: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(alignment: .leading) {
                Icon(icon, size: 40).foregroundStyle(iconColor)
                Spacer()
                VStack(alignment: .leading, spacing: 6) {
                    Text(title).font(Theme.sans(26, .semibold)).foregroundStyle(Theme.text).lineLimit(2).minimumScaleFactor(0.8)
                    Text(detail).font(Theme.sans(18)).foregroundStyle(Theme.muted).lineLimit(2)
                }
            }
            .padding(26)
            .frame(width: tileWidth, height: 230, alignment: .leading)
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.07))
        .focused($focus, equals: item)
    }
}

// MARK: - Réglages de la console

/// « Réglages de {nom} » : présentés comme ceux d'un ordinateur, avec ce qui a
/// du sens sur un téléviseur. Image : définition, images par seconde, débit,
/// format, agrandissement et netteté (MetalFX, tvOS 27.1 et suivants) ;
/// Commandes : intensités de la DualSense. Pas de plein écran (toujours), ni
/// de clavier et de souris (la partie se joue à la manette).
struct ConsoleSettingsView: View {
    let consoleID: UUID
    /// Ouverts depuis le menu de la partie : « Retour » y ramène.
    var close: (() -> Void)?
    @EnvironmentObject private var playstation: PlayStationStore
    @EnvironmentObject private var app: AppModel
    @State private var section = 0
    @State private var confirmRemove = false
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, section(Int), row(String) }

    private var console: PlayStationConsole? { playstation.console(consoleID) }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            if let console {
                content(console)
            }
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, .section(0))
        .onExitCommand {
            switch focus {
            case .row?: focus = .section(section)
            default: leave()
            }
        }
        .alert(PSTVText.removeQuestion(console?.name ?? ""), isPresented: $confirmRemove) {
            Button(StationText.removeConfirm, role: .destructive) {
                if let console { playstation.remove(console) }
                app.selected = app.store.ordered.first?.id ?? playstation.consoles.first?.id
                leave()
            }
            Button(StationText.cancel, role: .cancel) {}
        }
    }

    private func leave() {
        if let close { close() } else { app.screen = .home }
    }

    private func content(_ c: PlayStationConsole) -> some View {
        ZStack(alignment: .topLeading) {
            Button { leave() } label: {
                HStack(spacing: 10) {
                    Icon("arrow-left", size: 26, bold: true)
                    Text("Retour").font(Theme.sans(22, .medium))
                }
                .padding(.leading, 18).padding(.trailing, 26)
                .frame(height: 60)
            }
            .buttonStyle(GlassButton(radius: 30))
            .focused($focus, equals: .back)
            .padding(.leading, 96).padding(.top, 56)

            VStack(alignment: .leading, spacing: 10) {
                Text(LinkText.settingsTitle(c.name))
                    .font(Theme.display(48, 660)).tracking(-48 * 0.035)
                    .lineLimit(2).minimumScaleFactor(0.7)
                    .padding(.bottom, 16)
                VStack(alignment: .leading, spacing: 10) {
                    sectionButton(0, PSTVText.picture, summary(c))
                    sectionButton(1, PSTVText.controls, PSTVText.controlsSummary(playstation.settings(c)))
                    sectionButton(2, PSTVText.console, c.model)
                }
                .focusSection()
                .defaultFocus($focus, .section(section), priority: .userInitiated)
            }
            .frame(width: 520, alignment: .leading)
            .padding(.leading, 96).padding(.top, 150)
            .onChange(of: focus) { _, new in
                if case let .section(i)? = new { section = i }
            }

            ScrollView(.vertical, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 12) {
                    switch section {
                    case 0: pictureRows(c)
                    case 1: controlRows(c)
                    default: consoleRows(c)
                    }
                }
                .padding(.horizontal, 52).padding(.vertical, 44)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .focusSection()
            .defaultFocus($focus, entry, priority: .userInitiated)
            .id(section)
            .frame(width: 1920 - 700 - 96, height: 1080 - 150 - 64, alignment: .topLeading)
            .glass(30)
            .padding(.leading, 700).padding(.top, 150)
        }
    }

    private var entry: Target {
        switch section {
        case 0: return .row("res.auto")
        case 1: return .row("rumble.down")
        default: return .row("remove")
        }
    }

    private func summary(_ c: PlayStationConsole) -> String {
        let s = playstation.settings(c)
        return "\(PSSettings.resolutionLabel(s.resolution)) · \(PSText.fps(s.fps))"
    }

    private func sectionButton(_ i: Int, _ title: String, _ subtitle: String) -> some View {
        Button { section = i } label: {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).font(Theme.sans(26, .semibold))
                    Text(verbatim: subtitle).font(Theme.sans(18)).foregroundStyle(Theme.muted).lineLimit(1)
                }
                Spacer()
                Icon("arrow-right", size: 22, bold: true).foregroundStyle(Theme.faint)
            }
            .padding(.horizontal, 26)
            .frame(width: 520, height: 92)
        }
        .buttonStyle(GlassButton(radius: 20, selected: section == i, scale: 1.03, fill: section == i ? Color.white.opacity(0.1) : Color.clear))
        .focused($focus, equals: .section(i))
    }

    // MARK: Image

    @ViewBuilder private func pictureRows(_ c: PlayStationConsole) -> some View {
        let s = playstation.settings(c)
        Theme.eyebrow(PSTVText.pictureCaption).padding(.bottom, 8)
        seg("res", PSText.resolution, s.resolution == "auto" ? PSText.resolutionAutoHelp : "",
            PSSettings.resolutions.map { ($0, PSSettings.resolutionLabel($0)) }, s.resolution) { r in edit(c) { $0.resolution = r } }
        seg("fps", PSText.frameRate, "", [60, 30].map { ("\($0)", PSText.fps($0)) }, "\(s.fps)") { f in edit(c) { $0.fps = Int(f) ?? 60 } }
        stepper("bitrate", PSText.bitrate, s.bitrateKbps == nil ? PSText.bitrateAutoHelp : "",
                value: s.bitrateKbps.map { PSText.mbps($0 / 1000) } ?? PSText.auto,
                down: { edit(c) { $0.bitrateKbps = Self.bitrateStep($0.bitrateKbps, resolution: $0.resolution, by: -1) } },
                up: { edit(c) { $0.bitrateKbps = Self.bitrateStep($0.bitrateKbps, resolution: $0.resolution, by: 1) } })
        let ps4 = c.model == "PS4"
        seg("codec", PSText.format, ps4 ? PSText.formatPS4Help : s.codec == "h264" ? PSText.formatH264Help : s.codec == "auto" ? PSText.formatAutoHelp : "",
            PSSettings.codecs.map { ($0, $0 == "auto" ? PSText.auto : $0 == "h264" ? "H.264" : "H.265") }, ps4 ? "h264" : s.codec,
            disabled: ps4 ? ["auto", "h265"] : []) { k in edit(c) { $0.codec = k } }
        if Upscaling.available {
            toggle("upscale", PSText.upscale, PSTVText.upscaleHelp, s.upscale) { edit(c) { $0.upscale.toggle() } }
            stepper("sharpness", PSText.sharpness, "", value: PSText.percent(s.sharpness), disabled: !s.upscale,
                    down: { edit(c) { $0.sharpness = max(0, $0.sharpness - 5) } },
                    up: { edit(c) { $0.sharpness = min(100, $0.sharpness + 5) } })
        }
    }

    /// Débit : Auto, puis de 2 à 100 Mbit/s ; sous 2, Auto revient.
    private static func bitrateStep(_ kbps: Int?, resolution: String, by step: Int) -> Int? {
        let range = PSSettings.bitrateRange
        let current = kbps.map { $0 / 1000 } ?? PSSettings.autoBitrate(resolution) / 1000
        if kbps == nil { return step > 0 ? min(range.upperBound, current + 1) * 1000 : max(range.lowerBound, current - 1) * 1000 }
        let next = current + step * (current >= 20 ? 5 : 1)
        if next < range.lowerBound { return nil }
        return min(range.upperBound, next) * 1000
    }

    // MARK: Commandes

    @ViewBuilder private func controlRows(_ c: PlayStationConsole) -> some View {
        let s = playstation.settings(c)
        Theme.eyebrow(PSText.controller).padding(.bottom, 8)
        stepper("rumble", PSText.rumble, "", value: PSText.percent(s.rumble),
                down: { edit(c) { $0.rumble = max(0, $0.rumble - 10) } }, up: { edit(c) { $0.rumble = min(100, $0.rumble + 10) } })
        stepper("triggers", PSText.adaptiveTriggers, "", value: PSText.percent(s.triggers),
                down: { edit(c) { $0.triggers = max(0, $0.triggers - 10) } }, up: { edit(c) { $0.triggers = min(100, $0.triggers + 10) } })
        Text(PSTVText.controllerHelp).font(Theme.sans(19)).foregroundStyle(Theme.muted).padding(.horizontal, 24).padding(.top, 8)
    }

    // MARK: Console

    @ViewBuilder private func consoleRows(_ c: PlayStationConsole) -> some View {
        Theme.eyebrow(PSTVText.console).padding(.bottom, 8)
        let accounts = playstation.pairedAccounts(c).map { playstation.name($0) }
        if !accounts.isEmpty {
            Text(PSText.pairedWith(accounts)).font(Theme.sans(22)).foregroundStyle(Theme.text2).padding(.horizontal, 24).padding(.bottom, 8)
        }
        if !playstation.unpairedAccounts(hostID: c.hostID).isEmpty || accounts.isEmpty {
            Button { app.screen = .consolePair(AppModel.PairRequest(hostID: c.hostID)) } label: {
                rowLabel(accounts.isEmpty ? PSText.pair : PSText.pairAnother, "")
            }
            .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
            .focused($focus, equals: .row("pair"))
        }
        Button { confirmRemove = true } label: {
            rowLabel(PSTVText.removeConsole, "", color: Theme.dangerText)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .row("remove"))
    }

    // MARK: Lignes

    private func edit(_ c: PlayStationConsole, _ change: (inout PSSettings) -> Void) {
        var s = playstation.settings(c)
        change(&s)
        playstation.setSettings(s, for: c)
    }

    private func rowLabel(_ title: String, _ note: String, color: Color = Theme.text) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(Theme.sans(24, .semibold)).foregroundStyle(color)
            if !note.isEmpty {
                Text(note).font(Theme.sans(17)).foregroundStyle(Theme.muted).lineLimit(3).fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.horizontal, 24).padding(.vertical, 10)
        .frame(maxWidth: .infinity, minHeight: 80, alignment: .leading)
    }

    private func seg(_ id: String, _ title: String, _ note: String, _ options: [(key: String, label: String)], _ current: String,
                     disabled: [String] = [], pick: @escaping (String) -> Void) -> some View {
        HStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(Theme.sans(24, .semibold))
                if !note.isEmpty { Text(note).font(Theme.sans(17)).foregroundStyle(Theme.muted).lineLimit(3) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 8) {
                ForEach(options, id: \.key) { o in
                    Button { pick(o.key) } label: { Text(o.label) }
                        .buttonStyle(ChoiceButton(selected: current == o.key, height: 60, light: true))
                        .disabled(disabled.contains(o.key))
                        .focused($focus, equals: .row("\(id).\(o.key)"))
                }
            }
            .fixedSize()
            .focusSection()
        }
        .padding(.horizontal, 24).padding(.vertical, 10)
        .frame(minHeight: 80)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
    }

    private func toggle(_ id: String, _ title: String, _ note: String, _ on: Bool, flip: @escaping () -> Void) -> some View {
        Button(action: flip) {
            HStack(spacing: 24) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).font(Theme.sans(24, .semibold))
                    Text(note).font(Theme.sans(17)).foregroundStyle(Theme.muted).lineLimit(3).fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Toggle2(on: on)
            }
            .padding(.horizontal, 24).padding(.vertical, 10)
            .frame(minHeight: 80)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .row(id))
        .accessibilityValue(on ? Text("Activé") : Text("Désactivé"))
    }

    /// Une valeur réglée par « − » et « + » (débit, netteté, intensités).
    private func stepper(_ id: String, _ title: String, _ note: String, value: String, disabled: Bool = false,
                         down: @escaping () -> Void, up: @escaping () -> Void) -> some View {
        HStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(Theme.sans(24, .semibold)).foregroundStyle(disabled ? Theme.ghost : Theme.text)
                if !note.isEmpty { Text(note).font(Theme.sans(17)).foregroundStyle(Theme.muted) }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 14) {
                Button(action: down) { Text(verbatim: "−").font(Theme.sans(30, .semibold)).frame(width: 40) }
                    .buttonStyle(ChoiceButton(selected: false, height: 60, light: true))
                    .focused($focus, equals: .row("\(id).down"))
                Text(verbatim: value).font(Theme.mono(22)).foregroundStyle(disabled ? Theme.ghost : Theme.text).frame(minWidth: 150)
                Button(action: up) { Text(verbatim: "+").font(Theme.sans(30, .semibold)).frame(width: 40) }
                    .buttonStyle(ChoiceButton(selected: false, height: 60, light: true))
                    .focused($focus, equals: .row("\(id).up"))
            }
            .disabled(disabled)
            .fixedSize()
            .focusSection()
        }
        .padding(.horizontal, 24).padding(.vertical, 10)
        .frame(minHeight: 80)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
    }
}

/// Textes des consoles propres à l'Apple TV (link-ui, « Consoles
/// PlayStation », Apple TV) ; les autres sont ceux de toutes les apps (`PSText`).
enum PSTVText {
    static var menuHint: String { String(localized: "Retour ou Options + Create + L1 + R1 : menu") }
    static var menuClose: String { String(localized: "Retour ou Options + Create + L1 + R1 : fermer le menu") }
    static var quitDetail: String { String(localized: "La console reste allumée") }
    static var picture: String { String(localized: "Image") }
    static var controls: String { String(localized: "Commandes") }
    static var console: String { String(localized: "Console") }
    static var pictureCaption: String { String(localized: "Image · la console envoie du 1080p au plus") }
    static var upscaleHelp: String { String(localized: "La console envoie du 1080p au plus : l'image est agrandie aux pixels de votre téléviseur par la puce graphique de l'Apple TV, en SDR. Coupé de lui-même si cette Apple TV est trop lente.") }
    static var upscaleOff: String { String(localized: "Agrandissement coupé : Apple TV trop lente") }
    static func upscaled(_ w: Int, _ h: Int, ms: Double?) -> String {
        let cost = ms.map { " · \(LinkFormat.decimal($0)) ms" } ?? ""
        let size = "\(w) × \(h)"
        return String(localized: "Agrandie en \(size)") + cost
    }
    static var controllerHelp: String { String(localized: "Pour la DualSense. Sur l'Apple TV, les vibrations HD deviennent des vibrations simples.") }
    static func controlsSummary(_ s: PSSettings) -> String {
        String(localized: "Vibrations \(s.rumble) % · gâchettes \(s.triggers) %")
    }
    static var removeConsole: String { String(localized: "Retirer cette console") }
    static func removeQuestion(_ name: String) -> String { String(localized: "Retirer \(name) de cet appareil ?") }
}
