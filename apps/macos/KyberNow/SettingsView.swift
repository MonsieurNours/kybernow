// Réglages d'un ordinateur (charte v2 ; docs/link-ui.md) : l'image de
// l'ordinateur en fond, floutée ; en tête, son nom et le profil ; à gauche,
// les six rubriques (Image, Son, Commandes, Réseau, Statistiques, Station),
// à droite leurs cartes en verre. Chaque changement est enregistré tout de
// suite ; toucher à l'image ou au réseau rend le profil « Personnalisé ».

import AppKit
import GameController
import KyberKit
import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var app: AppModel
    let hostID: UUID

    var body: some View {
        if let host = app.store.host(hostID) {
            SettingsContent(host: host, status: app.store.status[hostID])
        } else {
            Color.clear.onAppear { app.route = .home }
        }
    }
}

/// Le profil choisi, ou « Personnalisé » quand les réglages s'en écartent.
private enum ProfilePick: Hashable {
    case profile(Profile)
    case custom
}

private struct SettingsContent: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let status: HostStatus?
    @State private var confirmRemove = false

    private var info: HostInfo? { status?.info }
    private var s: ConnectionSettings { host.settings }

    private func update(custom: Bool = false, _ change: (inout ConnectionSettings) -> Void) {
        var h = host
        change(&h.settings)
        if custom { h.settings.profile = nil }
        app.store.update(h)
    }

    private var profile: Binding<ProfilePick?> {
        Binding(get: { s.profile.map { .profile($0) } ?? .custom }, set: { p in
            if case let .profile(p)? = p { update { $0.apply(p) } }
        })
    }

    private var stationConfirm: Binding<StationConfirm?> {
        Binding(get: { app.store.stationConfirm?.hostID == host.id ? app.store.stationConfirm : nil },
                set: { app.store.stationConfirm = $0 })
    }

    var body: some View {
        ZStack {
            Theme.background.ignoresSafeArea()
            HostBackdrop(host: host, picture: app.store.imagery.pictures[host.id], online: status?.online, drift: false)
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
        .alert(stationConfirm.wrappedValue?.title ?? "", isPresented: Binding(get: { stationConfirm.wrappedValue != nil }, set: { if !$0 { app.store.stationConfirm = nil } }), presenting: stationConfirm.wrappedValue) { c in
            Button(c.confirmLabel, role: .destructive) {
                app.store.stationConfirm = nil
                if c.kind == .restart { app.store.restartStation(host, force: true) } else { app.store.updateStation(host, force: true) }
            }
            Button(StationText.cancel, role: .cancel) { app.store.stationConfirm = nil }
        } message: { c in
            Text(c.message)
        }
        .alert(StationText.removeQuestion(host.name), isPresented: $confirmRemove) {
            Button(StationText.removeConfirm, role: .destructive) {
                app.store.remove(host)
                app.selectedID = nil
                app.route = .home
            }
            Button(StationText.cancel, role: .cancel) {}
        }
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
            Text(LinkText.settingsTitle(host.name))
                .font(Theme.display(40, .bold)).tracking(-40 * 0.035)
                .foregroundStyle(Theme.text)
                .lineLimit(1).minimumScaleFactor(0.6)
                .accessibilityAddTraits(.isHeader)
            Spacer(minLength: 16)
            VStack(alignment: .trailing, spacing: 8) {
                HStack(spacing: 12) {
                    Theme.eyebrow(String(localized: "Profil"), color: Theme.muted)
                    Segmented(options: Profile.allCases.map { (ProfilePick.profile($0), $0.label) } + (s.profile == nil ? [(.custom, Profile.label(nil))] : []),
                              selection: profile,
                              icons: [.profile(.jeu): "game-controller", .profile(.bureau): "article", .profile(.max): "lightning", .custom: "sliders-horizontal"],
                              height: 36)
                        .fixedSize()
                }
                Text(Profile.hint(s.profile))
                    .font(Theme.sans(13)).foregroundStyle(Theme.muted)
            }
        }
    }

    // MARK: - Rubriques

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(ComputerSection.allCases) { section in
                let on = app.section == section
                Button { withAnimation(Theme.easeOut) { app.section = section } } label: {
                    HStack(spacing: 12) {
                        Icon(section.icon, size: 18)
                            .foregroundStyle(on ? Theme.accent : Theme.muted)
                            .frame(width: 32, height: 32)
                            .background(on ? Theme.accentSoft : Color.clear, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                        VStack(alignment: .leading, spacing: 2) {
                            Text(section.title).font(Theme.sans(14.5, .semibold)).foregroundStyle(on ? Theme.text : Theme.text2)
                            Text(verbatim: subtitle(section)).font(Theme.sans(12)).foregroundStyle(section == .station && updateVersion != nil ? Theme.accentLight : Theme.faint).lineLimit(1)
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
    private func subtitle(_ section: ComputerSection) -> String {
        switch section {
        case .picture:
            guard s.display != .physical, info?.virtualDisplay != false else { return String(localized: "Sans écran virtuel") }
            let definition = switch s.display {
            case .window: String(localized: "Taille de la fenêtre")
            case .custom: String(localized: "Sur mesure")
            default: s.limit4K ? String(localized: "Au plus 4K") : String(localized: "Comme cet écran")
            }
            return SectionSummary.picture(definition, refresh: sessionMode.refresh)
        case .sound: return s.audio.label
        case .controls: return String(localized: "Souris : \(s.mouse.label)")
        case .network: return s.transport.label
        case .statistics: return SectionSummary.statistics(host)
        case .station: return SectionSummary.station(info)
        }
    }

    @ViewBuilder private var detail: some View {
        Group {
            switch app.section {
            case .picture: pictureSection
            case .sound: soundSection
            case .controls: controlsSection
            case .network: networkSection
            case .statistics: statisticsSection
            case .station: stationSection
            }
        }
        .id(app.section)
        .transition(.opacity)
    }

    /// Deux colonnes de cartes quand la fenêtre est large, une sinon.
    private func columns<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 400), spacing: 18, alignment: .top)], alignment: .leading, spacing: 18) {
            content()
        }
    }

    // MARK: - Image

    private var pictureSection: some View {
        columns {
            displayCard
            pictureCard
            bitrateCard
        }
    }

    private var screen: NSScreen { NSScreen.main ?? NSScreen.screens[0] }

    private var displayCard: some View {
        SettingsCard(icon: "monitor", title: String(localized: "Écran")) {
            let m = DisplayInfo.mode(for: screen, codec: s.codec, limit4K: false)
            let capped = DisplayInfo.mode(for: screen, codec: s.codec, limit4K: true)
            let virtual = s.display != .physical
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Écran virtuel").font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                    Text(SettingsText.virtualDisplayHelp).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                Switch(isOn: Binding(get: { virtual }, set: { v in update { $0.display = v ? .match : .physical } }), label: String(localized: "Écran virtuel"))
            }
            if info != nil, info?.virtualDisplay == false, virtual {
                Notice(kind: .warn, text: String(localized: "Le pilote d'écran virtuel ne répond pas sur l'ordinateur : vous verrez ses écrans tels quels."))
            }
            VStack(alignment: .leading, spacing: 8) {
                Text("Définition").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                option(String(localized: "Comme cet écran"), "\(LinkFormat.dims(m.width, m.height)) · \(m.refresh) Hz", on: s.display == .match && !s.limit4K) {
                    update { $0.display = .match; $0.limit4K = false }
                }
                if m.width > 3840 || m.height > 2160 {
                    option(String(localized: "Au plus 4K"), "\(LinkFormat.dims(capped.width, capped.height)) · \(capped.refresh) Hz", on: s.display == .match && s.limit4K) {
                        update { $0.display = .match; $0.limit4K = true }
                    }
                }
                option(String(localized: "Taille de la fenêtre"), String(localized: "session fenêtrée, l'écran suit la fenêtre"), prose: true, on: s.display == .window) {
                    update { $0.display = .window }
                }
                option(String(localized: "Sur mesure"), "\(LinkFormat.dims(s.customWidth, s.customHeight)) · \(s.customRefresh) Hz", on: s.display == .custom) {
                    update { $0.display = .custom }
                }
                if s.display == .custom { customMode }
            }
            .disabled(!virtual)
            .opacity(virtual ? 1 : 0.45)
            screensRow
            refreshRow
                .disabled(!virtual)
                .opacity(virtual ? 1 : 0.45)
            if s.codec == .h264, DisplayInfo.mode(for: screen, codec: .h264).width < DisplayInfo.mode(for: screen, codec: .hevc).width {
                Text("En H.264, l'ordinateur encode au plus 4096 pixels de large.")
                    .font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    /// « Écrans » (docs/link-ui.md) : Auto, Cet écran, Tous les écrans ;
    /// inactif, sur « Cet écran », avec une définition sur mesure (ou une
    /// session fenêtrée) et sans écran virtuel. Ne rend pas le profil
    /// « Personnalisé ».
    private var screensRow: some View {
        let custom = s.display == .custom || s.display == .window
        // Écran virtuel demandé et utilisable sur la Station : la même règle que le cœur.
        let enabled = SessionEngine.screensSettingEnabled(customResolution: custom, virtualDisplay: s.display != .physical && info?.virtualDisplay != false)
        let shown: ScreensChoice = enabled ? s.screens : .this
        return VStack(alignment: .leading, spacing: 8) {
            Text(SettingsText.screensTitle).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
            Segmented(options: ScreensChoice.allCases.map { ($0, $0.label) },
                      selection: Binding(get: { shown }, set: { choice in
                          if let choice, enabled { update { $0.screens = choice } }
                      }),
                      height: 30, fill: true)
            if enabled || custom {
                Text(enabled ? SettingsText.screensHelp : SettingsText.screensCustomHelp)
                    .font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            }
        }
        .disabled(!enabled)
        .opacity(enabled ? 1 : 0.45)
    }

    private func option(_ title: String, _ sub: String, prose: Bool = false, on: Bool, pick: @escaping () -> Void) -> some View {
        Button(action: pick) {
            HStack(spacing: 12) {
                Circle()
                    .strokeBorder(on ? Theme.accent : Theme.line3, lineWidth: on ? 5 : 2)
                    .frame(width: 16, height: 16)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                    Text(sub).font(prose ? Theme.sans(12.5) : Theme.mono(12)).foregroundStyle(Theme.muted)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 14).padding(.vertical, 10)
            .background(on ? Theme.accentSoft.opacity(0.45) : Theme.well, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(on ? Theme.accent : Theme.line))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private var customMode: some View {
        HStack(spacing: 8) {
            number(s.customWidth, range: 640...7680) { v in update { $0.customWidth = v } }
            Text(verbatim: "×").foregroundStyle(Theme.muted)
            number(s.customHeight, range: 480...4320) { v in update { $0.customHeight = v } }
        }
        .font(Theme.sans(13))
        .padding(.leading, 28)
    }

    private func number(_ value: Int, range: ClosedRange<Int>, set: @escaping (Int) -> Void) -> some View {
        NumberField(value: value, range: range, set: set).frame(width: 72)
    }

    /// Fréquence : celle de cet écran, ou 60 Hz pour alléger l'encodage et le réseau.
    private var refreshRow: some View {
        let native = max(30, screen.maximumFramesPerSecond)
        let current = sessionMode.refresh
        let rates = Array(Set([native, 60, current].filter { $0 <= native || $0 == current })).sorted(by: >)
        return VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Fréquence").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                Spacer()
                if rates.count == 1 { Text(verbatim: "\(current) Hz").font(Theme.mono(13)).foregroundStyle(Theme.text) }
            }
            if rates.count > 1 {
                Segmented(options: rates.map { ($0, "\($0) Hz") },
                          selection: Binding(get: { current }, set: { hz in if let hz { pickRefresh(hz, native: native) } }),
                          height: 30, fill: true)
            }
            Text(variableRefresh
                 ? "L'écran de ce Mac se cale sur chaque image : latence minimale sans déchirure."
                 : "L'écran virtuel de l'ordinateur suit ; 60 Hz allège l'encodage et le réseau.")
                .font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
        }
    }

    private func pickRefresh(_ hz: Int, native: Int) {
        update { st in
            switch st.display {
            case .custom:
                st.customRefresh = hz
            case .match, .window, .physical:
                guard hz != native else { return }
                let m = DisplayInfo.mode(for: screen, codec: st.codec, limit4K: st.limit4K)
                st.display = .custom
                (st.customWidth, st.customHeight, st.customRefresh) = (m.width, m.height, hz)
            }
        }
    }

    private var pictureCard: some View {
        SettingsCard(icon: "image", title: String(localized: "Image")) {
            VStack(alignment: .leading, spacing: 8) {
                Text("Format vidéo").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                Segmented(
                    options: [(VideoCodec.hevc, "HEVC"), (.h264, "H.264"), (.av1, "AV1")],
                    selection: Binding(get: { s.codec }, set: { c in if let c { update(custom: true) { $0.codec = c } } }),
                    disabled: disabledCodecs,
                    fill: true
                )
                Text(codecHint).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            }
            toggleRow(String(localized: "Texte net (4:4:4)"),
                      can444 ? SettingsText.sharpTextHelp : (s.codec == .h264 ? String(localized: "Le 4:4:4 demande le HEVC.") : String(localized: "Ce Mac ne décode pas le 4:4:4 en matériel : l'image reste en 4:2:0.")),
                      isOn: Binding(get: { s.yuv444 && can444 }, set: { v in update(custom: true) { $0.yuv444 = v } }))
                .disabled(!can444)
            VStack(alignment: .leading, spacing: 8) {
                Text("Affichage des images").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                Segmented(options: [(true, String(localized: "Synchronisé")), (false, String(localized: "Immédiat"))],
                          selection: Binding(get: { s.vsync }, set: { v in if let v { update { $0.vsync = v } } }),
                          height: 30, fill: true)
                Text(s.vsync
                     ? "Chaque image attend le prochain rafraîchissement de l'écran : elle n'est jamais coupée."
                     : variableRefresh ? "Votre écran suit déjà l'image (fréquence variable) : en immédiat, vous ne gagnez presque rien."
                                       : "Chaque image s'affiche dès qu'elle arrive : un peu moins de délai, une coupure possible dans les mouvements rapides.")
                    .font(Theme.sans(12)).foregroundStyle(!s.vsync && variableRefresh ? Theme.warnText : Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            toggleRow(String(localized: "Garder la dernière image"),
                      String(localized: "À la fin de la session, une petite image de l'écran de l'ordinateur reste sur ce Mac : le fond de l'accueil."),
                      isOn: Binding(get: { s.keepLastImage }, set: { v in
                          update { $0.keepLastImage = v }
                          if !v { app.store.imagery.forgetLast(host) }
                      }))
        }
    }

    private var bitrateCard: some View {
        SettingsCard(icon: "lightning", title: String(localized: "Débit")) {
            BitrateControl(value: Binding(get: { s.bitrateMbps }, set: { v in update(custom: true) { $0.bitrateMbps = v } }))
            if s.profile != nil {
                let m = sessionMode
                let effective = s.bitrate(width: m.width, height: m.height, refresh: m.refresh)
                if effective != s.bitrateMbps {
                    Text("Débit du profil calé sur la 4K à 60 Hz : à \(LinkFormat.dims(m.width, m.height)) et \(m.refresh) Hz, la session part à \(effective) Mbit/s. Un débit réglé à la main reste tel quel.")
                        .font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    private func toggleRow(_ title: String, _ note: String, isOn: Binding<Bool>) -> some View {
        HStack(alignment: .top, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                Text(note).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
            Switch(isOn: isOn, label: title)
        }
    }

    private var can444: Bool { ConnectionSettings.decodes444(s.codec) }

    /// Mode probable de la session (l'ordinateur peut l'ajuster).
    private var sessionMode: (width: Int, height: Int, refresh: Int) {
        switch s.display {
        case .custom: return (s.customWidth, s.customHeight, s.customRefresh)
        default: return DisplayInfo.mode(for: screen, codec: s.codec, limit4K: s.limit4K)
        }
    }

    private var variableRefresh: Bool { DisplayInfo.variableRefresh(screen) }

    /// Formats que l'ordinateur encode ET que ce Mac décode en matériel.
    private var disabledCodecs: Set<VideoCodec> {
        let caps = DecoderCapabilities.current
        var off: Set<VideoCodec> = []
        if !caps.hevc { off.insert(.hevc) }
        if !(caps.av1 && info?.av1 == true) { off.insert(.av1) }
        return off
    }

    private var codecHint: String {
        let base: String
        switch s.codec {
        case .h264: base = String(localized: "Le plus compatible. Demande plus de débit pour la même netteté.")
        case .hevc: base = String(localized: "Recommandé : plus net à débit égal, décodé par la puce de ce Mac.")
        case .av1: base = String(localized: "Le plus efficace, décodé par les puces M3 et suivantes (en 4:2:0 seulement : pas de texte net).")
        }
        if !DecoderCapabilities.current.av1 { return base + " " + String(localized: "Ce Mac ne décode pas l'AV1 en matériel.") }
        return info?.av1 == true ? base : base + " " + String(localized: "AV1 demande une carte RTX 40 ou plus récente sur l'ordinateur.")
    }

    // MARK: - Son

    private var soundSection: some View {
        columns {
            SettingsCard(icon: "speaker-high", title: String(localized: "Son")) {
                Segmented(
                    options: AudioChoice.allCases.map { ($0, $0.label) },
                    selection: Binding(get: { s.audio }, set: { a in if let a { update { $0.audio = a } } }),
                    height: 30,
                    fill: true
                )
                Text(audioHint)
                    .font(Theme.sans(12))
                    .foregroundStyle(s.audio == .surround51 || s.audio == .surround71 ? Theme.warnText : Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var audioHint: String {
        if info != nil, info?.speakers == false, s.audio != .off {
            return s.audio.hint + " " + String(localized: "Sans Steam sur l'ordinateur, le son reste aussi sur ses haut-parleurs.")
        }
        return s.audio.hint
    }

    // MARK: - Commandes

    private var controlsSection: some View {
        columns {
            SettingsCard(icon: "cursor", title: String(localized: "Souris et clavier")) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Souris").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                    Segmented(
                        options: MouseChoice.allCases.map { ($0, $0.label) },
                        selection: Binding(get: { s.mouse }, set: { m in if let m { update { $0.mouse = m } } }),
                        height: 30,
                        fill: true
                    )
                    Text(s.mouse.hint).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
                    Text("Pendant la session, la barre en haut de l'écran change de souris, et ⌃⌥⌘M bascule.")
                        .font(Theme.sans(12)).foregroundStyle(Theme.faint).fixedSize(horizontal: false, vertical: true)
                }
                toggleRow(String(localized: "Touche ⌘ envoyée comme Ctrl"),
                          String(localized: "⌘C, ⌘V et les autres raccourcis du Mac font Ctrl+C, Ctrl+V sur l'ordinateur."),
                          isOn: Binding(get: { s.commandAsControl }, set: { v in update { $0.commandAsControl = v } }))
                toggleRow(String(localized: "Presse-papiers partagé"),
                          String(localized: "Le texte copié sur ce Mac se colle sur l'ordinateur, et inversement, jusqu'à 60 Ko. Admin peut l'interdire dans un sens ou dans l'autre."),
                          isOn: Binding(get: { s.clipboard }, set: { v in update { $0.clipboard = v } }))
            }
            SettingsCard(icon: "game-controller", title: String(localized: "Manette")) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Manette vue par l'ordinateur").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                    Segmented(
                        options: GamepadChoice.allCases.map { ($0, $0.label) },
                        selection: Binding(get: { s.gamepad }, set: { g in if let g { update { $0.gamepad = g } } }),
                        height: 30,
                        fill: true
                    )
                    Text(s.gamepad.hint).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
                    Text("Ce choix prime sur l'option « Toutes les manettes en Xbox 360 » d'Admin, qui reste le choix des autres appareils.")
                        .font(Theme.sans(12)).foregroundStyle(Theme.faint).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    // MARK: - Réseau

    private var networkSection: some View {
        columns {
            SettingsCard(icon: "wifi-high", title: String(localized: "Réseau")) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Transport de l'image").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                    Segmented(
                        options: Transport.allCases.map { ($0, $0.label) },
                        selection: Binding(get: { s.transport }, set: { t in if let t { update(custom: true) { $0.transport = t } } }),
                        fill: true
                    )
                    Text(Transport.help).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
                }
                toggleRow(String(localized: "Reconnexion automatique"),
                          String(localized: "Après une coupure, la session reprend où elle était."),
                          isOn: Binding(get: { s.autoReconnect }, set: { v in update { $0.autoReconnect = v } }))
            }
            SettingsCard(icon: "power", title: SettingsText.wakeTitle) {
                if let mac = SettingsText.wakeAddress(host) {
                    HStack(alignment: .firstTextBaseline) {
                        HStack(spacing: 8) {
                            Circle().fill(Theme.ok).frame(width: 7, height: 7)
                            Text(SettingsText.wakeReady).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                        }
                        Spacer(minLength: 12)
                        Text(verbatim: mac).font(Theme.mono(12)).foregroundStyle(Theme.text).textSelection(.enabled)
                    }
                } else {
                    Text(SettingsText.wakeNone).font(Theme.sans(13)).foregroundStyle(Theme.text2).fixedSize(horizontal: false, vertical: true)
                }
                Text(SettingsText.wakeHelp).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    // MARK: - Statistiques

    private var statisticsSection: some View {
        columns {
            LastSessionCard(summary: LastSessionSummary(host))
            ThisComputerCard(host: host, status: status)
        }
    }

    // MARK: - Station

    private var updateVersion: String? {
        guard let st = info?.station, st.updateAvailable else { return nil }
        return st.updateVersion
    }

    private var stationSection: some View {
        columns {
            StationCard(host: host, status: status)
            SettingsCard(icon: "globe-hemisphere-west", title: "Admin") {
                Text(StationText.adminHelp).font(Theme.sans(12.5)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
                Button {
                    if let url = HomeAdmin.url(for: host, store: app.store) { NSWorkspace.shared.open(url) }
                } label: {
                    HStack(spacing: 8) { Icon("arrow-right", size: 15, bold: true); Text(StationText.openAdmin) }
                }
                .buttonStyle(GhostButton(height: 36))
                .disabled(status?.online != true)
            }
            AccountCard(host: host, status: status) { confirmRemove = true }
        }
    }
}

/// Adresse d'Admin d'un ordinateur : celle du cœur ; à défaut (démonstration,
/// mot de passe absent), `https://adresse:port/`.
enum HomeAdmin {
    @MainActor
    static func url(for host: Host, store: HostStore) -> URL? {
        if let url = store.console(for: host)?.adminURL { return url }
        let address = host.address.contains(":") ? "[\(host.address)]" : host.address
        return URL(string: "https://\(address):\(host.consolePort)/")
    }
}

// MARK: - Statistiques

/// Dernière session : la médiane en grand, lue contre la durée d'une image
/// (règle de trame), le 95e centile, la durée, ce qui a été transmis.
struct LastSessionCard: View {
    let summary: LastSessionSummary?

    var body: some View {
        SettingsCard(icon: "chart-line", title: StatsText.lastSession) {
            if let summary {
                if let median = summary.median {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(verbatim: LinkFormat.decimal(median)).font(Theme.mono(34, .medium)).foregroundStyle(Theme.text)
                        Text(verbatim: "ms").font(Theme.mono(13)).foregroundStyle(Theme.muted)
                        Text(StatsText.median).font(Theme.mono(13)).foregroundStyle(Theme.muted)
                        Spacer(minLength: 8)
                        Text(verbatim: summary.when).font(Theme.sans(12.5)).foregroundStyle(Theme.muted)
                    }
                    FrameRuler(median: median, p95: summary.p95)
                        .padding(.top, 6)
                    if let p95 = summary.p95 {
                        Text(StatsText.under(p95)).font(Theme.sans(12.5)).foregroundStyle(Theme.text2)
                    }
                } else {
                    Text(verbatim: summary.when).font(Theme.sans(12.5)).foregroundStyle(Theme.muted)
                }
                if !summary.transmitted.isEmpty {
                    Text(verbatim: summary.transmitted).font(Theme.mono(12)).foregroundStyle(Theme.muted)
                }
                Text(StatsText.help).font(Theme.sans(12)).foregroundStyle(Theme.faint).fixedSize(horizontal: false, vertical: true)
            } else {
                Text(StatsText.none).font(Theme.sans(14)).foregroundStyle(Theme.text2)
                Text(StatsText.help).font(Theme.sans(12)).foregroundStyle(Theme.faint).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// Cet ordinateur : système, carte graphique, adresse d'Admin, empreinte ;
/// Station réinstallée : l'empreinte présentée, à comparer avant de la retenir.
private struct ThisComputerCard: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let status: HostStatus?
    @State private var presented: String?

    var body: some View {
        SettingsCard(icon: "desktop-tower", title: StatsText.thisComputer) {
            VStack(alignment: .leading, spacing: 10) {
                fact(StatsText.system, status?.info?.os ?? "—", mono: false)
                fact(StatsText.graphics, status?.info?.gpu ?? "—", mono: false)
                fact(StatsText.address, "\(host.address):\(host.consolePort)")
                fact(StatsText.fingerprint, host.consoleFingerprint.map(LinkFormat.fingerprint) ?? "—")
            }
            if status?.problem == .certificateChanged {
                if presented == nil {
                    Notice(kind: .warn, text: LinkText.problem(.certificateChanged))
                }
                if let presented {
                    fact(StatsText.presented, LinkFormat.fingerprint(presented))
                    Text(StatsText.compare).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
                    Button(StatsText.trust) { app.store.trust(fingerprint: presented, for: host) }
                        .buttonStyle(SignalButton(height: 34))
                }
            }
        }
        .task(id: status?.problem) {
            guard status?.problem == .certificateChanged else { return }
            presented = await app.store.presentedFingerprint(of: host)
        }
    }

    private func fact(_ label: String, _ value: String, mono: Bool = true) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).font(Theme.sans(13)).foregroundStyle(Theme.muted).fixedSize()
            Spacer(minLength: 12)
            Text(verbatim: value).font(mono ? Theme.mono(12) : Theme.sans(13)).foregroundStyle(Theme.text)
                .multilineTextAlignment(.trailing).fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
        }
    }
}

// MARK: - Station

/// La Station : sa version, sa mise à jour, le redémarrage, et ce qui est en cours.
private struct StationCard: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let status: HostStatus?

    private var station: StationInfo? { status?.info?.station }
    private var task: StationTask? { app.store.stationTasks[host.id] }
    private var working: Bool { task?.working == true || station?.restarting == true || station?.installing == true }
    private var online: Bool { status?.online == true }

    var body: some View {
        SettingsCard(icon: "desktop-tower", title: String(localized: "Station")) {
            HStack(alignment: .firstTextBaseline) {
                Text(verbatim: StationText.version(status?.info?.stationVersion ?? "—")).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
                Spacer(minLength: 12)
                if let station, !working {
                    if station.updateAvailable, let v = station.updateVersion {
                        badge(StationText.available(v), lit: true)
                    } else {
                        badge(StationText.upToDate, lit: false)
                    }
                }
            }
            if let task { line(task) }
            if status?.info != nil, station == nil {
                Notice(kind: .info, text: StationText.tooOld)
            } else if let station {
                action(StationText.restart, StationText.restartHelp) { app.store.restartStation(host) }
                if station.updateAvailable, let v = station.updateVersion {
                    if station.installable {
                        action(StationText.update, StationText.updateHelp(v), primary: true) { app.store.updateStation(host) }
                    } else if station.requiresSetup {
                        Notice(kind: .info, text: StationText.requiresSetup(v))
                    }
                }
            }
        }
    }

    private func badge(_ text: String, lit: Bool) -> some View {
        Text(text)
            .font(Theme.sans(12, .semibold))
            .foregroundStyle(lit ? Theme.accentLight : Theme.ok)
            .padding(.horizontal, 10).frame(height: 24)
            .background((lit ? Theme.accent : Theme.ok).opacity(0.14), in: Capsule())
    }

    @ViewBuilder private func line(_ task: StationTask) -> some View {
        HStack(spacing: 10) {
            switch task {
            case .restarting:
                ProgressView().controlSize(.small)
                Text(StationText.restarting).foregroundStyle(Theme.text)
            case .updating:
                ProgressView().controlSize(.small)
                Text(StationText.updating).foregroundStyle(Theme.text)
            case let .done(text):
                Icon("check-circle", size: 16, bold: true).foregroundStyle(Theme.ok)
                Text(text).foregroundStyle(Theme.text)
            case let .failed(text):
                Icon("warning", size: 16, bold: true).foregroundStyle(Theme.warn)
                Text(text).foregroundStyle(Theme.warnText).fixedSize(horizontal: false, vertical: true)
            }
        }
        .font(Theme.sans(13))
        .padding(.horizontal, 12).padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.well, in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.line))
    }

    @ViewBuilder private func action(_ title: String, _ help: String, primary: Bool = false, run: @escaping () -> Void) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            if primary {
                Button(action: run) { Text(title) }
                    .buttonStyle(PrimaryButton(height: 38))
                    .disabled(working || !online)
            } else {
                Button(action: run) {
                    HStack(spacing: 8) { Icon("arrows-clockwise", size: 15, bold: true); Text(title) }
                }
                .buttonStyle(GhostButton(height: 36))
                .disabled(working || !online)
            }
            Text(help).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
        }
        .opacity(working || !online ? 0.5 : 1)
    }
}

/// Le compte de cet appareil auprès d'Admin, et le retrait de l'ordinateur.
private struct AccountCard: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let status: HostStatus?
    let remove: () -> Void
    @State private var password = ""

    var body: some View {
        SettingsCard(icon: "user", title: StationText.account) {
            Text(verbatim: host.username).font(Theme.mono(15, .medium)).foregroundStyle(Theme.text).textSelection(.enabled)
            if app.store.ownDevice(host) {
                Text(StationText.ownAccess).font(Theme.sans(12)).foregroundStyle(Theme.muted)
            }
            if status?.problem == .passwordRefused {
                VStack(alignment: .leading, spacing: 6) {
                    Text(StationText.newPassword).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                    HStack(spacing: 8) {
                        SecureField(text: $password) { EmptyView() }
                            .textFieldStyle(.plain)
                            .font(Theme.sans(13))
                            .padding(.horizontal, 10)
                            .frame(height: 32)
                            .background(Theme.bar, in: RoundedRectangle(cornerRadius: 8))
                            .overlay(RoundedRectangle(cornerRadius: 8).stroke(Theme.line2))
                            .onSubmit(save)
                        Button(StationText.save, action: save)
                            .buttonStyle(SignalButton(height: 32))
                            .disabled(password.isEmpty)
                    }
                    Text(StationText.passwordHelp).font(Theme.sans(12)).foregroundStyle(Theme.muted)
                }
            }
            Rectangle().fill(Color.white.opacity(0.07)).frame(height: 1).padding(.vertical, 4)
            Button(action: remove) {
                HStack(spacing: 8) { Icon("trash", size: 15, bold: true); Text(StationText.remove + "…") }
            }
            .buttonStyle(GhostButton(height: 36, tint: Theme.dangerText, border: Theme.danger.opacity(0.35)))
        }
    }

    private func save() {
        guard !password.isEmpty else { return }
        app.store.updatePassword(password, for: host)
        password = ""
    }
}

// MARK: - Débit

/// Débit de 5 à 4 294 Mbit/s, curseur logarithmique, saisie libre et
/// avertissements au-delà de 500 et 900 Mbit/s.
private struct BitrateControl: View {
    @Binding var value: Int

    private let lo = Double(ConnectionSettings.minBitrate)
    private let hi = Double(ConnectionSettings.maxBitrate)

    private var position: Binding<Double> {
        Binding(
            get: { log(Double(value) / lo) / log(hi / lo) },
            set: { p in
                let v = lo * pow(hi / lo, p)
                value = v < 100 ? Int(v.rounded()) : v < 1000 ? Int((v / 5).rounded()) * 5 : Int((v / 50).rounded()) * 50
            }
        )
    }

    private var level: Int { value > 900 ? 2 : value > 500 ? 1 : 0 }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Débit").font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                Spacer()
                NumberField(value: value, range: ConnectionSettings.minBitrate...ConnectionSettings.maxBitrate) { value = $0 }
                    .frame(width: 76)
                Text(verbatim: "Mbit/s").font(Theme.sans(12)).foregroundStyle(Theme.muted)
            }
            Slider(value: position, in: 0...1)
                .tint(level == 2 ? Theme.danger : level == 1 ? Theme.warn : Theme.accent)
                .accessibilityLabel("Débit")
                .accessibilityValue("\(value) mégabits par seconde")
            HStack {
                ForEach(["5", "50", "300", "1 000", "4 294"], id: \.self) { t in
                    Text(t).font(Theme.mono(11)).foregroundStyle(Theme.faint)
                    if t != "4 294" { Spacer() }
                }
            }
            switch level {
            case 2:
                Notice(kind: .danger, text: String(localized: "Au-delà de ce qu'un réseau Gigabit transporte (~940 Mbit/s). Réservé au 2,5 ou 10 Gbit/s, câblé de bout en bout. Sinon, l'image saccadera."))
            case 1:
                Notice(kind: .warn, text: String(localized: "Très haut débit. En Wi-Fi ou sur un réseau chargé, des images risquent de se perdre et la latence de grimper. Préférez un câble Ethernet."))
            default:
                Text("Plus de débit, plus de détails dans les scènes chargées. En réseau local, 80 à 150 Mbit/s suffisent en 4K.")
                    .font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// Champ numérique compact, validé à la frappe d'Entrée ou en quittant le champ.
private struct NumberField: View {
    let value: Int
    let range: ClosedRange<Int>
    let set: (Int) -> Void
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        TextField(text: $text) { EmptyView() }
            .textFieldStyle(.plain)
            .font(Theme.mono(13))
            .multilineTextAlignment(.trailing)
            .foregroundStyle(Theme.text)
            .padding(.horizontal, 8)
            .frame(height: 30)
            .background(Theme.bar, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).stroke(focused ? Theme.accent : Theme.line2))
            .focused($focused)
            .onAppear { text = String(value) }
            .onChange(of: value) { _, v in if !focused { text = String(v) } }
            .onSubmit(commit)
            .onChange(of: focused) { _, f in if !f { commit() } }
    }

    private func commit() {
        let v = Int(text.filter(\.isNumber)) ?? value
        let clamped = min(max(v, range.lowerBound), range.upperBound)
        text = String(clamped)
        if clamped != value { set(clamped) }
    }
}

/// Manette branchée sur ce Mac (Réglages de l'app).
struct GamepadStatus: View {
    @State private var name: String?

    var body: some View {
        HStack(spacing: 10) {
            Circle().fill(name != nil ? Theme.ok : Theme.faint).frame(width: 8, height: 8)
            Text(name.map { String(localized: "Manette prête : \($0)") } ?? String(localized: "Aucune manette branchée"))
                .font(Theme.sans(13)).foregroundStyle(Theme.text2)
        }
        .padding(.horizontal, 12).padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.well, in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.line))
        .onAppear(perform: refresh)
        .onReceive(NotificationCenter.default.publisher(for: .GCControllerDidConnect)) { _ in refresh() }
        .onReceive(NotificationCenter.default.publisher(for: .GCControllerDidDisconnect)) { _ in refresh() }
    }

    private func refresh() {
        name = GCController.controllers().first?.vendorName
    }
}
