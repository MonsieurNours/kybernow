// Réglages d'un ordinateur (maquettes iOS, planche 7), présentés en fiche
// par RootView. Les mêmes réglages que sur Mac et Apple TV, en cinq sections :
// image, son, entrées, réseau, compte et iCloud. Sur iPhone, les sections en
// onglets qui défilent ; sur iPad (ou dès que la place le permet), les
// sections à gauche et le détail à droite, le profil en haut à droite.
//
// Chaque changement est enregistré tout de suite. Partir d'un profil ; toucher
// au format, au 4:4:4, au débit ou au transport le rend « Personnalisé ». Les
// choix que l'appareil ne sait pas faire restent visibles, grisés, avec la
// raison ; le débit affiché est celui de la session pour la définition en cours.

import AVFoundation
import KyberKit
import SwiftUI

struct SettingsView: View {
    var hostID: UUID
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore

    var body: some View {
        ZStack {
            AmbientBackground()
            if let host = store.host(hostID) {
                GeometryReader { g in
                    SettingsPane(host: host, info: store.status[hostID]?.info, size: g.size)
                }
            }
        }
        .preferredColorScheme(.dark)
        // iPad : la fiche prend presque tout l'écran, pour les sections à
        // gauche et le détail à droite (une fiche de formulaire est trop étroite).
        .presentationSizing(SettingsSheetSizing())
    }
}

/// Taille de la fiche des réglages : tout l'espace que le système accorde
/// (iPad) ; sur iPhone, une fiche a de toute façon la largeur de l'écran.
struct SettingsSheetSizing: PresentationSizing {
    func proposedSize(for root: PresentationSizingRoot, context: PresentationSizingContext) -> ProposedViewSize {
        ProposedViewSize(width: .infinity, height: .infinity)
    }
}

/// Les cinq sections.
enum SettingsSection: Int, CaseIterable, Identifiable {
    case image, sound, inputs, network, account

    var id: Int { rawValue }

    var title: String {
        switch self {
        case .image: return String(localized: "Image")
        case .sound: return String(localized: "Son")
        case .inputs: return String(localized: "Entrées")
        case .network: return String(localized: "Réseau")
        case .account: return String(localized: "Compte et iCloud")
        }
    }
}

private struct SettingsPane: View {
    let host: Host
    let info: HostInfo?
    let size: CGSize

    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @ObservedObject private var prefs = Preferences.shared
    @StateObject private var link = SettingsLink()
    @State private var section: SettingsSection = .image
    @State private var route = AudioRoute.current
    @State private var confirmRemove = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var s: ConnectionSettings { host.settings }

    /// Sections à gauche, détail à droite.
    private var twoColumns: Bool { size.width >= 760 }
    private var sideWidth: CGFloat { min(380, max(280, size.width * 0.3)) }
    /// Lignes en largeur : le détail a la place d'un libellé et de ses choix côte à côte.
    private var wideRows: Bool { twoColumns && size.width - sideWidth - 2 * gutter - 28 - 72 >= 600 }
    private var gutter: CGFloat { size.width >= 1100 ? 72 : 40 }
    private var rows: SettingsRows { SettingsRows(wide: wideRows) }

    var body: some View {
        Group {
            if twoColumns { wideLayout } else { compactLayout }
        }
        .onReceive(NotificationCenter.default.publisher(for: AVAudioSession.routeChangeNotification).receive(on: RunLoop.main)) { _ in
            route = .current
        }
        .alert(DeviceDisplay.isPad ? Text("Retirer \(host.name) de cet iPad ?") : Text("Retirer \(host.name) de cet iPhone ?"), isPresented: $confirmRemove) {
            Button(role: .destructive) {
                store.remove(host)
                app.selected = store.ordered.first?.id
                app.sheet = nil
            } label: { Text("Retirer") }
            Button(role: .cancel) {} label: { Text("Annuler") }
        } message: {
            Text("Ses accès sont effacés d'ici. L'ordinateur reste sur vos autres appareils.")
        }
    }

    // MARK: - Mises en page

    /// iPhone : navigation, titre, profil, onglets, détail.
    private var compactLayout: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Button { app.sheet = nil } label: {
                    HStack(spacing: 4) {
                        Icon("arrow-left", size: 20, bold: true)
                        Text("Vos ordinateurs").font(Theme.sans(16))
                    }
                    .foregroundStyle(Theme.accentLight)
                    .padding(.leading, 6).padding(.trailing, 12)
                    .frame(minHeight: 44)
                    .contentShape(Rectangle())
                }
                .buttonStyle(SettingsPressStyle())
                Spacer(minLength: 12)
                Text(host.name.uppercased())
                    .font(Theme.mono(10.5))
                    .tracking(0.8)
                    .foregroundStyle(Theme.faint)
                    .lineLimit(1)
            }
            .padding(.leading, 12).padding(.trailing, 16)
            .padding(.top, 8)

            VStack(alignment: .leading, spacing: 12) {
                Text("Réglages")
                    .font(Theme.display(34))
                    .tracking(-34 * 0.03)
                    .foregroundStyle(Theme.text)
                    .accessibilityAddTraits(.isHeader)
                SettingsProfilePicker(current: s.profile, fill: true) { p in update { $0.apply(p) } }
                Text(profileSummary)
                    .font(Theme.sans(12))
                    .foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 24)
            .padding(.top, 8)

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(SettingsSection.allCases) { sec in
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

    /// iPad : retour et profil en haut, sections à gauche, détail à droite.
    private var wideLayout: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Button { app.sheet = nil } label: {
                    HStack(spacing: 8) {
                        Icon("arrow-left", size: 18, bold: true)
                        Text("Vos ordinateurs")
                    }
                }
                .buttonStyle(GlassButton(height: 44, fontSize: 15))
                Spacer(minLength: 16)
                Text("Profil de \(host.name)")
                    .font(Theme.sans(14))
                    .foregroundStyle(Theme.muted)
                    .lineLimit(1)
                SettingsProfilePicker(current: s.profile, fill: false) { p in update { $0.apply(p) } }
                    .fixedSize()
            }
            .padding(.horizontal, gutter)
            .padding(.top, 28)

            HStack(alignment: .top, spacing: 28) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Réglages")
                            .font(Theme.display(44))
                            .tracking(-44 * 0.035)
                            .foregroundStyle(Theme.text)
                            .padding(.bottom, 16)
                            .accessibilityAddTraits(.isHeader)
                        ForEach(SettingsSection.allCases) { sec in
                            sectionButton(sec)
                        }
                        Text(profileSummary)
                            .font(Theme.sans(13))
                            .foregroundStyle(Theme.muted)
                            .fixedSize(horizontal: false, vertical: true)
                            .padding(.horizontal, 4)
                            .padding(.top, 14)
                        deviceCard.padding(.top, 4)
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

    private func sectionButton(_ sec: SettingsSection) -> some View {
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
            .background(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(on ? Color.white.opacity(0.1) : Color.clear)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .strokeBorder(on ? Color.white.opacity(0.16) : Color.clear)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(SettingsPressStyle())
        .hoverEffect(.highlight)
        .accessibilityAddTraits(on ? [.isSelected, .isButton] : .isButton)
    }

    /// Ce que l'appareil sait faire : les profils s'y adaptent.
    private var deviceCard: some View {
        let n = DeviceDisplay.native()
        let caps = DecoderCapabilities.current
        let parts = [
            "\(n.width) × \(n.height) à \(n.refresh) Hz",
            String(localized: "décodage matériel : \(caps.summary)"),
            caps.hevc444 ? String(localized: "4:4:4 testé au lancement : oui") : String(localized: "4:4:4 testé au lancement : non"),
            link.label,
        ]
        return VStack(alignment: .leading, spacing: 6) {
            Theme.eyebrow(DeviceDisplay.isPad ? String(localized: "Cet iPad") : String(localized: "Cet iPhone"))
            Text(verbatim: parts.joined(separator: " · "))
                .font(Theme.sans(14))
                .foregroundStyle(Theme.text2)
                .lineSpacing(3)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 18).padding(.vertical, 16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Color.white.opacity(0.04)))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Color.white.opacity(0.08)))
    }

    private func select(_ sec: SettingsSection) {
        if reduceMotion {
            section = sec
        } else {
            withAnimation(Theme.easeOut) { section = sec }
        }
    }

    private var profileSummary: String {
        s.profile?.summary ?? String(localized: "Personnalisé : vous avez ajusté un profil.")
    }

    /// Résumé d'une section, sous son titre (iPad).
    private func subtitle(_ sec: SettingsSection) -> String {
        switch sec {
        case .image:
            return "\(DeviceDisplay.label(mode.width, mode.height)) · \(mode.refresh) Hz · \(effectiveBitrate) Mbit/s"
        case .sound:
            if s.audio == .off { return String(localized: "Sur l'ordinateur") }
            return "\(s.audio == .auto ? route.channelsLabel : s.audio.label) · \(route.name) · \(route.latencyMs) ms"
        case .inputs:
            var parts = [prefs.directTouch ? String(localized: "Toucher direct") : String(localized: "Pavé"), String(localized: "souris \(s.mouse.label)")]
            if s.commandAsControl { parts.append(String(localized: "⌘ en Ctrl")) }
            return parts.joined(separator: " · ")
        case .network:
            return "\(s.transport.label) · \(link.label)"
        case .account:
            return prefs.icloudSync ? String(localized: "\(host.username) · synchronisé") : host.username
        }
    }

    // MARK: - Détail

    /// « 2622 × 1206 », sans séparateur de milliers (une chaîne localisée
    /// écrirait « 2 622 » en français).
    static func dims(_ width: Int, _ height: Int) -> String {
        "\(String(width)) × \(String(height))"
    }

    private var caption: String {
        switch section {
        case .image:
            let n = DeviceDisplay.native()
            return DeviceDisplay.isPad
                ? String(localized: "Image · cet iPad : \(Self.dims(n.width, n.height)), \(n.refresh) Hz")
                : String(localized: "Image · cet iPhone : \(Self.dims(n.width, n.height)), \(n.refresh) Hz")
        case .sound:
            return String(localized: "Son · sortie : \(route.name)")
        case .inputs:
            return DeviceDisplay.isPad
                ? String(localized: "Entrées · toucher, trackpad, souris, manette, clavier")
                : String(localized: "Entrées · toucher, souris, manette, clavier")
        case .network:
            return String(localized: "Réseau · \(link.label)")
        case .account:
            return String(localized: "Compte et iCloud · \(host.username)")
        }
    }

    private var detail: some View {
        VStack(alignment: .leading, spacing: wideRows ? 10 : 8) {
            Theme.eyebrow(caption)
                .padding(.top, 2)
                .padding(.bottom, 6)
            switch section {
            case .image: imageRows
            case .sound: soundRows
            case .inputs: inputRows
            case .network: networkRows
            case .account: accountRows
            }
        }
        .id(section)
        .transition(reduceMotion ? .opacity : .opacity.combined(with: .offset(x: 16)))
    }

    // MARK: - Image

    private var native: (width: Int, height: Int, refresh: Int) { DeviceDisplay.native() }
    private var mode: (width: Int, height: Int, refresh: Int) { DeviceDisplay.mode(for: s) }

    /// Définition « comme cet écran », ramenée à la 4K au plus.
    private func matchMode(limit4K: Bool) -> (width: Int, height: Int) {
        var t = s
        t.display = .match
        t.limit4K = limit4K
        let m = DeviceDisplay.mode(for: t)
        return (m.width, m.height)
    }

    private var effectiveBitrate: Int {
        s.bitrate(width: mode.width, height: mode.height, refresh: mode.refresh)
    }

    private var definitionKey: String {
        switch s.display {
        case .physical:
            return "physical"
        case .custom:
            let dims = (s.customWidth / 2 * 2, s.customHeight / 2 * 2)
            let full = matchMode(limit4K: false), capped = matchMode(limit4K: true)
            let isFull = dims == (full.width, full.height), isCapped = dims == (capped.width, capped.height)
            if isFull && isCapped { return s.limit4K ? "4k" : "native" }
            return isFull ? "native" : isCapped ? "4k" : "custom"
        case .match, .window:
            return s.limit4K ? "4k" : "native"
        }
    }

    private func pickDefinition(_ key: String) {
        update { st in
            if key == "physical" {
                st.display = .physical
                return
            }
            if key == "custom" { return }
            let hz = st.display == .physical ? native.refresh : DeviceDisplay.mode(for: st).refresh
            st.limit4K = key == "4k"
            if hz >= native.refresh {
                st.display = .match
            } else {
                var t = st
                t.display = .match
                let m = DeviceDisplay.mode(for: t)
                st.display = .custom
                (st.customWidth, st.customHeight, st.customRefresh) = (m.width, m.height, hz)
            }
        }
    }

    private func pickRefresh(_ hz: Int) {
        update { st in
            var t = st
            t.display = .match
            let m = DeviceDisplay.mode(for: t)
            let dims = st.display == .custom ? (st.customWidth / 2 * 2, st.customHeight / 2 * 2) : (m.width, m.height)
            if hz >= native.refresh, dims == (m.width, m.height) {
                st.display = .match
            } else {
                st.display = .custom
                (st.customWidth, st.customHeight, st.customRefresh) = (dims.0, dims.1, hz)
            }
        }
    }

    @ViewBuilder private var imageRows: some View {
        let n = native
        let key = definitionKey
        let full = matchMode(limit4K: false)
        let capped = matchMode(limit4K: true)
        let definitions = definitionOptions(key)
        let definitionNote: String = switch key {
        case "physical": String(localized: "L'ordinateur montre ses écrans tels quels, avec des bandes noires si leur format diffère.")
        case "custom": String(localized: "Sur mesure, choisie sur un autre appareil.")
        default: n.width > 3840
            ? String(localized: "Comme cet écran : \(Self.dims(full.width, full.height)). Au plus 4K : \(Self.dims(capped.width, capped.height)), plus léger à encoder et à transporter.")
            : String(localized: "Comme cet écran : \(Self.dims(full.width, full.height)). Au plus 4K : pour un écran externe.")
        }
        rows.choice(String(localized: "Définition"), definitionNote, definitions, key, pick: pickDefinition)

        let rates = refreshOptions
        rows.choice(String(localized: "Fréquence"), refreshNote, rates, "\(mode.refresh)", enabled: s.display != .physical) { v in
            if let hz = Int(v) { pickRefresh(hz) }
        }

        rateRow

        let caps = DecoderCapabilities.current
        let av1Missing = !caps.av1
        let av1Host = info?.av1 == false
        rows.choice(String(localized: "Format"), codecNote, [
            SettingsOption(key: VideoCodec.hevc.rawValue, label: "HEVC", unavailable: !caps.hevc),
            SettingsOption(key: VideoCodec.h264.rawValue, label: "H.264"),
            SettingsOption(key: VideoCodec.av1.rawValue, label: "AV1", unavailable: av1Missing || av1Host),
        ], s.codec.rawValue) { v in
            if let c = VideoCodec(rawValue: v) { update(custom: true) { $0.codec = c } }
        }

        let can444 = ConnectionSettings.decodes444(s.codec)
        rows.toggle(String(localized: "Texte net (4:4:4)"), yuvNote, s.yuv444 && can444, enabled: can444) {
            update(custom: true) { $0.yuv444 = !($0.yuv444 && can444) }
        }
    }

    /// Comme cet écran, au plus 4K, sans écran virtuel ; une définition sur
    /// mesure (choisie sur Mac) reste affichée tant qu'elle est retenue.
    private func definitionOptions(_ key: String) -> [SettingsOption] {
        var options = [
            SettingsOption(key: "native", label: String(localized: "Comme cet écran")),
            SettingsOption(key: "4k", label: String(localized: "Au plus 4K")),
        ]
        if key == "custom" {
            options.append(SettingsOption(key: "custom", label: "\(s.customWidth) × \(s.customHeight)"))
        }
        options.append(SettingsOption(key: "physical", label: String(localized: "Sans écran virtuel")))
        return options
    }

    private var refreshOptions: [SettingsOption] {
        let n = native
        var rates = [n.refresh]
        if n.refresh > 60 { rates.append(60) }
        if !rates.contains(mode.refresh) { rates.append(mode.refresh) }
        var options = rates.sorted(by: >).map { SettingsOption(key: "\($0)", label: "\($0) Hz") }
        if n.refresh <= 60 { options.insert(SettingsOption(key: "120", label: "120 Hz", unavailable: true), at: 0) }
        return options
    }

    private var refreshNote: String {
        if s.display == .physical { return String(localized: "Sans écran virtuel, l'ordinateur garde la fréquence de ses écrans.") }
        if native.refresh <= 60 {
            return DeviceDisplay.isPad
                ? String(localized: "120 Hz : cet iPad affiche au plus 60 images par seconde.")
                : String(localized: "120 Hz : cet iPhone affiche au plus 60 images par seconde.")
        }
        return String(localized: "L'écran virtuel de l'ordinateur suit ; 60 Hz allège l'encodage et le réseau.")
    }

    private var codecNote: String {
        let caps = DecoderCapabilities.current
        if !caps.av1 {
            return DeviceDisplay.isPad
                ? String(localized: "AV1 : cet iPad ne le décode pas en matériel.")
                : String(localized: "AV1 : cet iPhone ne le décode pas en matériel.")
        }
        if info?.av1 == false { return String(localized: "AV1 : il faut une carte RTX 40 ou plus récente sur l'ordinateur.") }
        switch s.codec {
        case .h264: return String(localized: "Le plus compatible. Demande plus de débit pour la même netteté.")
        case .av1: return String(localized: "Le plus efficace, en 4:2:0 seulement : pas de texte net.")
        case .hevc:
            return DeviceDisplay.isPad
                ? String(localized: "Cet iPad décode les trois en matériel. HEVC : net à débit égal, recommandé.")
                : String(localized: "Cet iPhone décode les trois en matériel. HEVC : net à débit égal, recommandé.")
        }
    }

    private var yuvNote: String {
        switch s.codec {
        case .h264: return String(localized: "Le 4:4:4 demande le HEVC.")
        case .av1: return String(localized: "Pas en AV1 : les décodeurs Apple ne lisent que le 4:2:0.")
        case .hevc:
            if ConnectionSettings.decodes444(.hevc) { return String(localized: "Pour le bureau : texte, code et tableurs nets ; plus de débit.") }
            return DeviceDisplay.isPad
                ? String(localized: "Cet iPad ne décode pas le 4:4:4 en matériel : l'image reste en 4:2:0.")
                : String(localized: "Cet iPhone ne décode pas le 4:4:4 en matériel : l'image reste en 4:2:0.")
        }
    }

    // MARK: Débit

    /// Au-delà, le lien du moment risque de perdre des images.
    private var comfortable: Int {
        switch link.kind {
        case .ethernet: return 900
        case .cellular: return 30
        default: return DeviceDisplay.isPad ? 120 : 80
        }
    }

    /// Données consommées en une heure à ce débit, en gigaoctets.
    private var gigabytesPerHour: String {
        let gb = Double(effectiveBitrate) * 3600 / 8 / 1000
        return gb >= 10 ? String(Int(gb.rounded())) : StatsFormat.decimal(gb)
    }

    private var rateNote: (text: String, warn: Bool) {
        let rate = effectiveBitrate
        let limit = comfortable
        switch link.kind {
        case .ethernet:
            return rate > limit
                ? (String(localized: "Au-delà de ce qu'un réseau Gigabit transporte (~940 Mbit/s). Réservé au 2,5 ou 10 Gbit/s, câblé de bout en bout."), true)
                : (String(localized: "Câble Ethernet : jusqu'à 900 Mbit/s sur un réseau Gigabit."), false)
        case .cellular:
            return rate > limit
                ? (String(localized: "Gros débit en données mobiles : environ \(gigabytesPerHour) Go par heure, et des images perdues si le réseau faiblit."), true)
                : (String(localized: "En données mobiles : environ \(gigabytesPerHour) Go par heure à ce débit."), false)
        default:
            return rate > limit
                ? (String(localized: "Au-delà de \(limit) Mbit/s en Wi-Fi, des images peuvent se perdre. Gardez-le pour le Wi-Fi 6 ou 7 près de la borne, ou un adaptateur Ethernet."), true)
                : (String(localized: "En Wi-Fi, \(limit / 2) à \(limit) Mbit/s suffisent pour cet écran à \(mode.refresh) Hz."), false)
        }
    }

    private func step(_ rate: Int) -> Int {
        rate < 50 ? 5 : rate < 200 ? 10 : rate < 1000 ? 50 : 250
    }

    /// Débit réel de la session ; le régler à la main part de lui et rend
    /// le profil « Personnalisé ».
    private func nudge(_ direction: Int) {
        let rate = effectiveBitrate
        let next = direction > 0 ? rate + step(rate) : rate - step(max(ConnectionSettings.minBitrate, rate - 1))
        let clamped = min(max(next, ConnectionSettings.minBitrate), ConnectionSettings.maxBitrate)
        update(custom: true) { $0.bitrateMbps = clamped }
    }

    private var rateRow: some View {
        let rate = effectiveBitrate
        let note = rateNote
        let top = Double(comfortable) * 2.5
        let wide = wideRows
        let gauge = GeometryReader { g in
            ZStack(alignment: .leading) {
                HStack(spacing: 0) {
                    Rectangle().fill(Color.white.opacity(0.14)).frame(width: g.size.width * 0.4)
                    Rectangle().fill(Theme.warn.opacity(0.35))
                }
                Rectangle()
                    .fill(note.warn ? Theme.warn : Theme.accent)
                    .frame(width: g.size.width * min(1, CGFloat(Double(rate) / top)))
            }
            .clipShape(Capsule())
        }
        .frame(height: 8)
        .accessibilityHidden(true)
        let buttons = HStack(spacing: 6) {
            Button { nudge(-1) } label: { Text(verbatim: "−").font(Theme.sans(wide ? 22 : 20)) }
                .buttonStyle(SettingsStepStyle(size: wide ? 48 : 44))
                .disabled(rate <= ConnectionSettings.minBitrate)
                .accessibilityLabel(Text("Moins de débit"))
            Button { nudge(1) } label: { Text(verbatim: "+").font(Theme.sans(wide ? 22 : 20)) }
                .buttonStyle(SettingsStepStyle(size: wide ? 48 : 44))
                .disabled(rate >= ConnectionSettings.maxBitrate)
                .accessibilityLabel(Text("Plus de débit"))
        }
        let real = s.profile != nil
            ? String(localized: "réel pour \(Self.dims(mode.width, mode.height)) à \(mode.refresh) Hz")
            : String(localized: "réglé à la main")
        let title = HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text("Débit").font(Theme.sans(wide ? 16 : 15, .semibold)).foregroundStyle(Theme.text)
            Text(verbatim: "\(rate) Mbit/s").font(Theme.mono(15)).foregroundStyle(Theme.accentLight)
                .contentTransition(.numericText())
            if wide {
                Text(verbatim: real).font(Theme.mono(11)).foregroundStyle(Theme.faint).lineLimit(1)
            }
        }
        let text = Text(note.text)
            .font(Theme.sans(wide ? 12.5 : 12))
            .foregroundStyle(note.warn ? Theme.warnText : Theme.muted)
            .lineSpacing(1.5)
            .fixedSize(horizontal: false, vertical: true)
        return rows.container {
            if wide {
                HStack(spacing: 20) {
                    VStack(alignment: .leading, spacing: 8) {
                        title
                        gauge
                        text
                    }
                    buttons
                }
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    HStack(alignment: .firstTextBaseline) {
                        title
                        Spacer(minLength: 8)
                        Text(verbatim: real).font(Theme.mono(10)).foregroundStyle(Theme.faint).lineLimit(1).minimumScaleFactor(0.8)
                    }
                    gauge
                    HStack(spacing: 10) {
                        text.frame(maxWidth: .infinity, alignment: .leading)
                        buttons
                    }
                }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Débit"))
        .accessibilityValue(Text(verbatim: "\(rate) Mbit/s"))
    }

    // MARK: - Son

    @ViewBuilder private var soundRows: some View {
        rows.choice(String(localized: "Canaux"), channelsNote.text, AudioChoice.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.audio.rawValue,
                    noteColor: channelsNote.warn ? Theme.warnText : Theme.muted) { v in
            if let a = AudioChoice(rawValue: v) { update { $0.audio = a } }
        }
        rows.toggle(String(localized: "Audio spatial dans les AirPods"), String(localized: "Le 5.1 ou le 7.1 du jeu, autour de vous ; sans suivi de la tête."),
                    prefs.spatialAudio, enabled: s.audio != .off) {
            prefs.spatialAudio.toggle()
            route = .current
        }
        rows.toggle(String(localized: "Prévenir d'un son en retard"), String(localized: "AirPods, AirPlay : alerte au-delà de 40 ms ajoutées."), prefs.latencyAlert) {
            prefs.latencyAlert.toggle()
        }
        rows.value(String(localized: "Retard mesuré de la sortie"), String(localized: "Mis à jour à chaque changement de sortie."), "\(route.latencyMs) ms",
                   noteColor: route.isLate ? Theme.warnText : Theme.muted)
    }

    private var channelsNote: (text: String, warn: Bool) {
        switch s.audio {
        case .auto: return (String(localized: "Auto : \(route.channelsLabel) sur \(route.name) ; 5.1 ou 7.1 vers un ampli ou en audio spatial."), false)
        case .stereo: return (String(localized: "Deux canaux : au casque ou sur le haut-parleur."), false)
        case .surround51, .surround71:
            let needed = s.audio == .surround51 ? 6 : 8
            if route.channels >= needed { return (String(localized: "\(s.audio.label) vers \(route.name)."), false) }
            return (String(localized: "\(s.audio.label) : il faut une sortie qui le porte (ampli HDMI ou USB-C) ou l'audio spatial ; sinon, le son est mixé en stéréo."), true)
        case .off: return (String(localized: "Le son reste sur l'ordinateur : ses haut-parleurs ou son casque."), false)
        }
    }

    // MARK: - Entrées

    @ViewBuilder private var inputRows: some View {
        rows.choice(String(localized: "Toucher"), String(localized: "Pavé : tout l'écran déplace le pointeur. Direct : le doigt le place."), [
            SettingsOption(key: "trackpad", label: String(localized: "Pavé tactile")),
            SettingsOption(key: "direct", label: String(localized: "Toucher direct")),
        ], prefs.directTouch ? "direct" : "trackpad") { v in
            prefs.directTouch = v == "direct"
        }
        if !prefs.directTouch {
            rows.choice(String(localized: "Vitesse du pointeur"), String(localized: "Au pavé tactile et à la manette."), [
                SettingsOption(key: "1.0", label: String(localized: "Lente")),
                SettingsOption(key: "1.4", label: String(localized: "Normale")),
                SettingsOption(key: "2.0", label: String(localized: "Rapide")),
            ], speedKey) { v in
                if let speed = Double(v) { prefs.pointerSpeed = speed }
            }
        }
        rows.choice(DeviceDisplay.isPad ? String(localized: "Souris et trackpad") : String(localized: "Souris"), mouseNote,
                    MouseChoice.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.mouse.rawValue) { v in
            if let m = MouseChoice(rawValue: v) { update { $0.mouse = m } }
        }
        rows.choice(String(localized: "Manette vue par l'ordinateur"), s.gamepad.hint,
                    GamepadChoice.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.gamepad.rawValue) { v in
            if let g = GamepadChoice(rawValue: v) { update { $0.gamepad = g } }
        }
        rows.toggle(String(localized: "Vibrations"), String(localized: "Renvoyées par le jeu, sur chaque manette."), prefs.rumble) {
            prefs.rumble.toggle()
        }
        rows.toggle(String(localized: "⌘ envoyée comme Ctrl"), String(localized: "Clavier externe : ⌘C fait Ctrl+C sur l'ordinateur."), s.commandAsControl) {
            update { $0.commandAsControl.toggle() }
        }
        rows.toggle(String(localized: "Clavier Apple ISO"), String(localized: "Clavier Apple européen : la touche sous Échap et celle près de Maj retrouvent leur place."), prefs.appleISOKeyboard) {
            prefs.appleISOKeyboard.toggle()
        }
        rows.toggle(String(localized: "Presse-papiers partagé"), String(localized: "Le texte copié ici se colle sur l'ordinateur, et inversement."), s.clipboard) {
            update { $0.clipboard.toggle() }
        }
    }

    private var speedKey: String {
        let speeds: [(String, Double)] = [("1.0", 1.0), ("1.4", 1.4), ("2.0", 2.0)]
        return speeds.min { abs($0.1 - prefs.pointerSpeed) < abs($1.1 - prefs.pointerSpeed) }?.0 ?? "1.4"
    }

    private var mouseNote: String {
        switch s.mouse {
        case .auto: return String(localized: "Auto : mode jeu quand un jeu est en plein écran sur l'ordinateur, bureau le reste du temps.")
        case .desktop: return String(localized: "Bureau : le pointeur se pose où vous le placez, toujours.")
        case .game: return String(localized: "Jeu : déplacements bruts, pointeur caché, pour viser.")
        }
    }

    // MARK: - Réseau

    @ViewBuilder private var networkRows: some View {
        rows.choice(String(localized: "Transport"), s.transport.hint, Transport.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.transport.rawValue) { v in
            if let t = Transport(rawValue: v) { update(custom: true) { $0.transport = t } }
        }
        rows.toggle(String(localized: "Se reconnecter tout seul"), String(localized: "Après une coupure, la session reprend où elle était."), s.autoReconnect) {
            update { $0.autoReconnect.toggle() }
        }
        rows.toggle(String(localized: "Autoriser les données mobiles"), String(localized: "Par VPN : environ \(gigabytesPerHour) Go par heure à ce débit."), prefs.allowCellular,
                    noteColor: prefs.allowCellular ? Theme.warnText : Theme.muted) {
            prefs.allowCellular.toggle()
        }
        let last = host.lastLatencyMs.map { " · " + StatsFormat.ms($0) } ?? ""
        rows.value(String(localized: "Qualité du lien"),
                   host.lastLatencyMs == nil ? String(localized: "Lien de cet appareil en ce moment.") : String(localized: "Lien de cet appareil ; latence médiane de la dernière session."),
                   link.label + last)
    }

    // MARK: - Compte et iCloud

    @ViewBuilder private var accountRows: some View {
        rows.toggle(String(localized: "Synchroniser avec iCloud"), String(localized: "Ordinateurs et accès suivent votre Mac et vos autres appareils."), prefs.icloudSync) {
            prefs.icloudSync.toggle()
        }
        rows.toggle(String(localized: "Garder la dernière image"),
                    DeviceDisplay.isPad ? String(localized: "Le fond de l'accueil, sur cet iPad seulement.") : String(localized: "Le fond de l'accueil, sur cet iPhone seulement."),
                    s.keepLastImage) {
            let keep = !s.keepLastImage
            update { $0.keepLastImage = keep }
            if !keep { store.imagery.forgetLast(host) }
        }
        let hasPassword = store.password(for: host) != nil
        rows.value(String(localized: "Compte"),
                   hasPassword
                       ? (DeviceDisplay.isPad ? String(localized: "Mot de passe dans le Trousseau de cet iPad.") : String(localized: "Mot de passe dans le Trousseau de cet iPhone."))
                       : String(localized: "Mot de passe introuvable : retirez puis ajoutez de nouveau l'ordinateur."),
                   host.username, noteColor: hasPassword ? Theme.muted : Theme.warnText)
        rows.value(String(localized: "Adresse"),
                   host.consoleFingerprint == nil ? String(localized: "Adresse de KyberNow Admin.") : String(localized: "Adresse de KyberNow Admin ; certificat retenu au premier contact."),
                   "\(host.address):\(host.consolePort)")
        rows.value(DeviceDisplay.isPad ? String(localized: "Retirer \(host.name) de cet iPad") : String(localized: "Retirer \(host.name) de cet iPhone"),
                   String(localized: "Ses accès sont effacés d'ici."), "", color: Theme.dangerText) {
            confirmRemove = true
        }
    }

    // MARK: - Enregistrement

    /// Enregistre un changement ; `custom` rend le profil « Personnalisé ».
    private func update(custom: Bool = false, _ change: (inout ConnectionSettings) -> Void) {
        guard var h = store.host(host.id) else { return }
        change(&h.settings)
        if custom { h.settings.profile = nil }
        store.update(h)
    }
}

/// Bouton − et + du débit.
private struct SettingsStepStyle: ButtonStyle {
    var size: CGFloat
    @Environment(\.isEnabled) private var enabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(enabled ? Theme.text : Theme.ghost)
            .frame(width: size, height: size)
            .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.white.opacity(configuration.isPressed ? 0.14 : 0.08)))
            .contentShape(Rectangle())
    }
}
