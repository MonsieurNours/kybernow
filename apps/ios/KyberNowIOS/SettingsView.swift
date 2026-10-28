// Réglages d'un ordinateur (maquettes iOS, planche 7 ; docs/link-ui.md),
// présentés en fiche par RootView : « Réglages de {nom} », le profil en tête,
// puis les six rubriques communes à toutes les apps KyberNow Link, dans cet
// ordre : Image, Son, Commandes, Réseau, Statistiques, Station. Sur iPhone,
// les rubriques en onglets qui défilent ; sur iPad (ou dès que la place le
// permet), les rubriques à gauche et le détail à droite, le profil en haut.
//
// Chaque changement est enregistré tout de suite. Partir d'un profil ; toucher
// au format, au 4:4:4, au débit ou au transport le rend « Personnalisé ». Les
// choix que l'appareil ne sait pas faire restent visibles, grisés, avec la
// raison ; le débit affiché est celui de la session pour la définition en
// cours. Ce qui vaut pour toute l'app (iCloud, langue, données mobiles, ce que
// l'appareil sait décoder) est dans les Réglages de l'app (AppSettingsView).

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
        // iPad : la fiche prend presque tout l'écran, pour les rubriques à
        // gauche et le détail à droite (une fiche de formulaire est trop étroite).
        .presentationSizing(SettingsSheetSizing())
        // Un appareil est en session : redémarrer ou mettre à jour quand même ?
        .alert(Text(confirm?.title ?? ""), isPresented: Binding(get: { confirm != nil }, set: { if !$0 { store.stationConfirm = nil } }), presenting: confirm) { c in
            Button(c.confirmLabel, role: .destructive) {
                guard let host = store.host(c.hostID) else { return }
                if c.kind == .restart { store.restartStation(host, force: true) } else { store.updateStation(host, force: true) }
            }
            Button(StationText.cancel, role: .cancel) {}
        } message: { c in
            Text(c.message)
        }
    }

    private var confirm: StationConfirm? {
        store.stationConfirm.flatMap { $0.hostID == hostID ? $0 : nil }
    }
}

/// Taille de la fiche des réglages : tout l'espace que le système accorde
/// (iPad) ; sur iPhone, une fiche a de toute façon la largeur de l'écran.
struct SettingsSheetSizing: PresentationSizing {
    func proposedSize(for root: PresentationSizingRoot, context: PresentationSizingContext) -> ProposedViewSize {
        ProposedViewSize(width: .infinity, height: .infinity)
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
    @State private var section: ComputerSection
    @State private var route = AudioRoute.current
    @State private var confirmRemove = false
    @State private var newPassword = ""
    /// Station réinstallée : l'empreinte qu'Admin présente maintenant.
    @State private var presented: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.openURL) private var openURL

    init(host: Host, info: HostInfo?, size: CGSize) {
        self.host = host
        self.info = info
        self.size = size
        #if DEBUG
        _section = State(initialValue: AppModel.demoSection ?? .picture)
        #else
        _section = State(initialValue: .picture)
        #endif
    }

    private var s: ConnectionSettings { host.settings }

    /// Rubriques à gauche, détail à droite.
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
        .alert(Text(StationText.removeQuestion(host.name)), isPresented: $confirmRemove) {
            Button(role: .destructive) {
                store.remove(host)
                app.selected = store.ordered.first?.id
                app.sheet = nil
            } label: { Text(StationText.removeConfirm) }
            Button(role: .cancel) {} label: { Text(StationText.cancel) }
        }
        .task(id: store.status[host.id]?.problem) {
            guard store.status[host.id]?.problem == .certificateChanged else { presented = nil; return }
            #if DEBUG
            if store.demo { presented = "sha256:c41e88a07d2b93f6e5a1c0b7d8e9f2a3b4c5d6e7f8091a2b3c4d5e6f7a8b9c0d"; return }
            #endif
            presented = await store.presentedFingerprint(of: host)
        }
    }

    // MARK: - Mises en page

    private var title: some View {
        Text(LinkText.settingsTitle(host.name))
            .font(Theme.display(twoColumns ? 40 : 30))
            .tracking(twoColumns ? -40 * 0.035 : -30 * 0.03)
            .foregroundStyle(Theme.text)
            .lineLimit(2)
            .minimumScaleFactor(0.7)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityAddTraits(.isHeader)
    }

    private var backButton: some View {
        Button { app.sheet = nil } label: {
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

    /// iPhone : retour, titre, profil, onglets des rubriques, détail.
    private var compactLayout: some View {
        VStack(alignment: .leading, spacing: 0) {
            backButton
                .padding(.leading, 12)
                .padding(.top, 8)

            VStack(alignment: .leading, spacing: 12) {
                title
                Theme.eyebrow(String(localized: "Profil"))
                    .padding(.top, 2)
                SettingsProfilePicker(current: s.profile, fill: true) { p in update { $0.apply(p) } }
                Text(Profile.hint(s.profile))
                    .font(Theme.sans(12))
                    .foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 24)
            .padding(.top, 4)

            ScrollViewReader { proxy in
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(ComputerSection.allCases) { sec in
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
                            .id(sec)
                        }
                    }
                    .padding(.horizontal, 24)
                }
                .onAppear { proxy.scrollTo(section, anchor: .center) }
                .onChange(of: section) { _, sec in withAnimation(Theme.easeOut) { proxy.scrollTo(sec, anchor: .center) } }
            }
            .padding(.top, 16)

            ScrollViewReader { proxy in
                ScrollView {
                    detail
                        .padding(.horizontal, 24)
                        .padding(.top, 14)
                        .padding(.bottom, 34)
                }
                .scrollBounceBehavior(.basedOnSize)
                .onAppear { demoScroll(proxy) }
            }
        }
    }

    /// iPad : retour et profil en haut, rubriques à gauche, détail à droite.
    private var wideLayout: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Button { app.sheet = nil } label: {
                    HStack(spacing: 8) {
                        Icon("arrow-left", size: 18, bold: true)
                        Text("Retour")
                    }
                }
                .buttonStyle(GlassButton(height: 44, fontSize: 15))
                Spacer(minLength: 16)
                Text("Profil")
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
                        title.padding(.bottom, 16)
                        ForEach(ComputerSection.allCases) { sec in
                            sectionButton(sec)
                        }
                        Text(Profile.hint(s.profile))
                            .font(Theme.sans(13))
                            .foregroundStyle(Theme.muted)
                            .fixedSize(horizontal: false, vertical: true)
                            .padding(.horizontal, 4)
                            .padding(.top, 14)
                    }
                    .padding(.bottom, 32)
                }
                .scrollBounceBehavior(.basedOnSize)
                .frame(width: sideWidth)

                ScrollViewReader { proxy in
                    ScrollView {
                        detail
                            .padding(.horizontal, 36)
                            .padding(.vertical, 32)
                    }
                    .scrollBounceBehavior(.basedOnSize)
                    .onAppear { demoScroll(proxy) }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                .glass(26, tint: 0.58)
                .clipShape(RoundedRectangle(cornerRadius: 26, style: .continuous))
            }
            .padding(.horizontal, gutter)
            .padding(.top, 34)
            .padding(.bottom, 40)
        }
    }

    private func sectionButton(_ sec: ComputerSection) -> some View {
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

    private func select(_ sec: ComputerSection) {
        if reduceMotion {
            section = sec
        } else {
            withAnimation(Theme.easeOut) { section = sec }
        }
    }

    /// Résumé d'une rubrique, sous son titre (iPad) : un choix simple, dans
    /// la langue de l'app, sans chiffre technique (SectionSummary).
    private func subtitle(_ sec: ComputerSection) -> String {
        switch sec {
        case .picture:
            guard s.display != .physical else { return LinkText.screen(width: 0, height: 0, refresh: 0, virtual: false, sizedForThisScreen: false) }
            let label = definitionOptions(definitionKey).first { $0.key == definitionKey }?.label ?? String(localized: "Comme cet écran")
            return SectionSummary.picture(label, refresh: mode.refresh)
        case .sound:
            return s.audio.label
        case .controls:
            return prefs.directTouch ? String(localized: "Toucher direct") : String(localized: "Pavé tactile")
        case .network:
            return s.transport.label
        case .statistics:
            return SectionSummary.statistics(host)
        case .station:
            return info == nil ? LinkText.status(store.status[host.id]?.online, away: store.status[host.id]?.away == true) : SectionSummary.station(info)
        }
    }

    // MARK: - Détail

    /// « 2622 × 1206 », sans séparateur de milliers (une chaîne localisée
    /// écrirait « 2 622 » en français).
    static func dims(_ width: Int, _ height: Int) -> String {
        LinkFormat.dims(width, height)
    }

    private var caption: String {
        switch section {
        case .picture:
            let n = DeviceDisplay.native()
            return DeviceDisplay.isPad
                ? String(localized: "Image · cet iPad : \(Self.dims(n.width, n.height)), \(n.refresh) Hz")
                : String(localized: "Image · cet iPhone : \(Self.dims(n.width, n.height)), \(n.refresh) Hz")
        case .sound:
            return String(localized: "Son · sortie : \(route.name)")
        case .controls:
            return DeviceDisplay.isPad
                ? String(localized: "Commandes · toucher, trackpad, souris, manette, clavier")
                : String(localized: "Commandes · toucher, souris, manette, clavier")
        case .network, .statistics, .station:
            return section.title
        }
    }

    private var detail: some View {
        VStack(alignment: .leading, spacing: wideRows ? 10 : 8) {
            Theme.eyebrow(caption)
                .padding(.top, 2)
                .padding(.bottom, 6)
            switch section {
            case .picture: imageRows
            case .sound: soundRows
            case .controls: controlRows
            case .network: networkRows
            case .statistics: statisticsRows
            case .station: stationRows
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
        case .custom:
            let dims = (s.customWidth / 2 * 2, s.customHeight / 2 * 2)
            let full = matchMode(limit4K: false), capped = matchMode(limit4K: true)
            let isFull = dims == (full.width, full.height), isCapped = dims == (capped.width, capped.height)
            if isFull && isCapped { return s.limit4K ? "4k" : "native" }
            return isFull ? "native" : isCapped ? "4k" : "custom"
        case .match, .window, .physical:
            return s.limit4K ? "4k" : "native"
        }
    }

    private func pickDefinition(_ key: String) {
        update { st in
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
        let virtual = s.display != .physical
        let definitionNote: String = if !virtual {
            String(localized: "Sans écran virtuel, l'ordinateur garde ses écrans tels quels.")
        } else if key == "custom" {
            String(localized: "Sur mesure, choisie sur un autre appareil.")
        } else if n.width > 3840 {
            String(localized: "Comme cet écran : \(Self.dims(full.width, full.height)). Au plus 4K : \(Self.dims(capped.width, capped.height)), plus léger à encoder et à transporter.")
        } else {
            String(localized: "Comme cet écran : \(Self.dims(full.width, full.height)). Au plus 4K : pour un écran externe.")
        }
        rows.choice(String(localized: "Définition"), definitionNote, definitionOptions(key), key, enabled: virtual, pick: pickDefinition)

        rows.choice(String(localized: "Fréquence"), refreshNote, refreshOptions, "\(mode.refresh)", enabled: virtual) { v in
            if let hz = Int(v) { pickRefresh(hz) }
        }

        let driverMissing = info?.virtualDisplay == false && virtual
        rows.toggle(String(localized: "Écran virtuel"),
                    driverMissing ? String(localized: "Le pilote d'écran virtuel ne répond pas sur l'ordinateur : vous verrez ses écrans tels quels.") : SettingsText.virtualDisplayHelp,
                    virtual, noteColor: driverMissing ? Theme.warnText : Theme.muted) {
            update { $0.display = $0.display == .physical ? .match : .physical }
        }

        let caps = DecoderCapabilities.current
        let av1Missing = !caps.av1
        let av1Host = info?.av1 == false
        rows.choice(String(localized: "Format vidéo"), codecNote, [
            SettingsOption(key: VideoCodec.hevc.rawValue, label: "HEVC", unavailable: !caps.hevc),
            SettingsOption(key: VideoCodec.h264.rawValue, label: "H.264"),
            SettingsOption(key: VideoCodec.av1.rawValue, label: "AV1", unavailable: av1Missing || av1Host),
        ], s.codec.rawValue) { v in
            if let c = VideoCodec(rawValue: v) { update(custom: true) { $0.codec = c } }
        }

        let can444 = ConnectionSettings.decodes444(s.codec)
        rows.toggle(String(localized: "Texte net (4:4:4)"), can444 ? SettingsText.sharpTextHelp : yuvNote, s.yuv444 && can444, enabled: can444) {
            update(custom: true) { $0.yuv444 = !($0.yuv444 && can444) }
        }

        rateRow

        rows.choice(String(localized: "Affichage des images"),
                    s.vsync ? String(localized: "Synchronisé : l'image n'est jamais coupée.") : String(localized: "Immédiat : un peu moins de délai ; l'image peut se couper dans les mouvements rapides."),
                    [SettingsOption(key: "sync", label: String(localized: "Synchronisé")), SettingsOption(key: "immediate", label: String(localized: "Immédiat"))],
                    s.vsync ? "sync" : "immediate") { v in
            update { $0.vsync = v == "sync" }
        }

        rows.toggle(String(localized: "Garder la dernière image"),
                    DeviceDisplay.isPad ? String(localized: "Le fond de l'accueil, sur cet iPad seulement.") : String(localized: "Le fond de l'accueil, sur cet iPhone seulement."),
                    s.keepLastImage) {
            let keep = !s.keepLastImage
            update { $0.keepLastImage = keep }
            if !keep { store.imagery.forgetLast(host) }
        }
    }

    /// Comme cet écran, au plus 4K ; une définition sur mesure (choisie sur
    /// Mac) reste affichée tant qu'elle est retenue.
    private func definitionOptions(_ key: String) -> [SettingsOption] {
        var options = [
            SettingsOption(key: "native", label: String(localized: "Comme cet écran")),
            SettingsOption(key: "4k", label: String(localized: "Au plus 4K")),
        ]
        if key == "custom" {
            options.append(SettingsOption(key: "custom", label: String(localized: "Sur mesure")))
        }
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
        rows.choice(String(localized: "Son"), channelsNote.text, AudioChoice.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.audio.rawValue,
                    noteColor: channelsNote.warn ? Theme.warnText : Theme.muted) { v in
            if let a = AudioChoice(rawValue: v) { update { $0.audio = a } }
        }
        rows.toggle(String(localized: "Audio spatial dans les AirPods"), String(localized: "Le 5.1 ou le 7.1 du jeu, autour de vous ; sans suivi de la tête."),
                    prefs.spatialAudio, enabled: s.audio != .off) {
            prefs.spatialAudio.toggle()
            route = .current
        }
        rows.toggle(String(localized: "Prévenir d'un son en retard"), String(localized: "AirPods, enceintes Bluetooth, AirPlay : une alerte au-delà de 40 ms ajoutées."), prefs.latencyAlert) {
            prefs.latencyAlert.toggle()
        }
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

    // MARK: - Commandes

    @ViewBuilder private var controlRows: some View {
        rows.choice(String(localized: "Toucher"), String(localized: "Pavé tactile : tout l'écran déplace le pointeur ; toucher clique, deux doigts font le clic droit et la molette. Toucher direct : le pointeur va là où le doigt touche."), [
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
        screenPadRows
        rows.choice(String(localized: "Manette vue par l'ordinateur"), s.gamepad.hint,
                    GamepadChoice.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.gamepad.rawValue) { v in
            if let g = GamepadChoice(rawValue: v) { update { $0.gamepad = g } }
        }
        rows.toggle(String(localized: "Vibrations"), PadHaptics.available
                        ? String(localized: "Renvoyées par le jeu, sur chaque manette, et sur l'iPhone quand la manette à l'écran est affichée.")
                        : String(localized: "Renvoyées par le jeu, sur chaque manette."), prefs.rumble) {
            prefs.rumble.toggle()
        }
        rows.toggle(String(localized: "Barre avec L1 + R1 + Start + Select"), String(localized: "Ouvre la barre de session depuis la manette, sans toucher l'écran."), prefs.menuCombo) {
            prefs.menuCombo.toggle()
        }
        rows.toggle(String(localized: "⌘ envoyée comme Ctrl"), String(localized: "Clavier externe : ⌘C fait Ctrl+C sur l'ordinateur."), s.commandAsControl) {
            update { $0.commandAsControl.toggle() }
        }
        rows.toggle(String(localized: "Clavier Apple ISO"), String(localized: "Clavier Apple européen (touche @# ou §± sous Échap) : la touche sous Échap et celle près de Maj retrouvent leur place sur l'ordinateur."), prefs.appleISOKeyboard) {
            prefs.appleISOKeyboard.toggle()
        }
        rows.toggle(String(localized: "Presse-papiers partagé"), String(localized: "Le texte copié ici se colle sur l'ordinateur, et inversement."), s.clipboard) {
            update { $0.clipboard.toggle() }
        }
    }

    /// Manette à l'écran (bouton de la barre de session) : disposition,
    /// taille, opacité, retour haptique (sans moteur haptique : une ligne le dit).
    @ViewBuilder private var screenPadRows: some View {
        Theme.eyebrow(ScreenPadText.name)
            .padding(.top, 12)
            .padding(.bottom, 2)
            .id("screenPad")
        rows.choice(String(localized: "Disposition"), String(localized: "Complète : une manette Xbox entière. Simple : le stick gauche, la croix, A, B, X, Y et Menu. Le petit bouton en haut de la manette passe de l'une à l'autre."),
                    ScreenPadLayout.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.screenPadLayout.rawValue) { v in
            if let l = ScreenPadLayout(rawValue: v) { update { $0.screenPadLayout = l } }
        }
        rows.choice(String(localized: "Taille"), String(localized: "Toucher deux fois un stick l'enfonce (L3, R3). Plus le doigt touche une gâchette bas, plus elle s'enfonce."),
                    ScreenPadSize.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.screenPadSize.rawValue) { v in
            if let size = ScreenPadSize(rawValue: v) { update { $0.screenPadSize = size } }
        }
        rows.slider(String(localized: "Opacité"), String(localized: "Un bouton touché s'allume toujours, même quand la manette est très transparente."),
                    value: s.screenPadOpacity, in: 0.2...1, step: 0.05, display: Self.percent) { v in
            update { $0.screenPadOpacity = v }
        }
        if PadHaptics.available {
            rows.choice(String(localized: "Retour haptique"), String(localized: "Un clic à chaque appui ; un tic quand un stick quitte le centre, change de direction ou arrive en butée."),
                        ScreenPadHaptics.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.screenPadHaptics.rawValue) { v in
                if let h = ScreenPadHaptics(rawValue: v) { update { $0.screenPadHaptics = h } }
            }
        } else {
            rows.value(String(localized: "Retour haptique"), DeviceDisplay.isPad
                       ? String(localized: "Cet iPad n'a pas de moteur haptique : seul le retour visuel reste.")
                       : String(localized: "Cet iPhone n'a pas de moteur haptique : seul le retour visuel reste."), "")
        }
    }

    /// Captures (`-scrollTo screenPad`) : le détail défile jusqu'au bloc voulu.
    private func demoScroll(_ proxy: ScrollViewProxy) {
        #if DEBUG
        guard let target = UserDefaults.standard.string(forKey: "scrollTo") else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { proxy.scrollTo(target, anchor: .top) }
        #endif
    }

    /// « 60 % », “60%”.
    private static func percent(_ v: Double) -> String {
        let n = Int((v * 100).rounded())
        return LanguageChoice.effective == "fr" ? "\(n) %" : "\(n)%"
    }

    private var speedKey: String {
        let speeds: [(String, Double)] = [("1.0", 1.0), ("1.4", 1.4), ("2.0", 2.0)]
        return speeds.min { abs($0.1 - prefs.pointerSpeed) < abs($1.1 - prefs.pointerSpeed) }?.0 ?? "1.4"
    }

    private var mouseNote: String {
        switch s.mouse {
        case .auto: return String(localized: "Auto : souris des jeux quand un jeu est en plein écran sur l'ordinateur, souris du bureau le reste du temps.")
        case .desktop: return String(localized: "Souris du bureau : le pointeur se pose où vous le placez, toujours.")
        case .game: return String(localized: "Souris des jeux : déplacements bruts, pointeur caché, pour viser.")
        }
    }

    // MARK: - Réseau

    @ViewBuilder private var networkRows: some View {
        rows.choice(String(localized: "Transport de l'image"), Transport.help, Transport.allCases.map { SettingsOption(key: $0.rawValue, label: $0.label) }, s.transport.rawValue) { v in
            if let t = Transport(rawValue: v) { update(custom: true) { $0.transport = t } }
        }
        rows.toggle(String(localized: "Reconnexion automatique"), String(localized: "Après une coupure, la session reprend où elle était."), s.autoReconnect) {
            update { $0.autoReconnect.toggle() }
        }
        wakeRow
    }

    /// « Réveil par le réseau » : prêt avec l'adresse MAC apprise de la
    /// Station, ou pas encore d'adresse.
    private var wakeRow: some View {
        rows.container {
            VStack(alignment: .leading, spacing: 10) {
                rows.label(SettingsText.wakeTitle, SettingsText.wakeHelp)
                if let mac = SettingsText.wakeAddress(host) {
                    HStack(spacing: 8) {
                        Circle().fill(Theme.ok).frame(width: 7, height: 7)
                        Text(SettingsText.wakeReady).font(Theme.sans(wideRows ? 14 : 13.5, .semibold)).foregroundStyle(Theme.text)
                        Text(verbatim: mac).font(Theme.mono(13)).foregroundStyle(Theme.text2).lineLimit(1).minimumScaleFactor(0.8)
                    }
                } else {
                    Text(SettingsText.wakeNone)
                        .font(Theme.sans(wideRows ? 13 : 12.5)).foregroundStyle(Theme.faint)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .accessibilityElement(children: .combine)
    }

    // MARK: - Statistiques

    @ViewBuilder private var statisticsRows: some View {
        LastSessionCard(summary: LastSessionSummary(host), wide: wideRows)
        Theme.eyebrow(StatsText.thisComputer)
            .padding(.top, 18)
            .padding(.bottom, 4)
        rows.value(StatsText.system, "", info?.os ?? "—")
        rows.value(StatsText.graphics, "", info?.gpu ?? "—")
        rows.value(StatsText.address, "", "\(host.address):\(String(host.consolePort))")
        rows.value(StatsText.fingerprint, "", host.consoleFingerprint.map(LinkFormat.fingerprint) ?? "—")
        if store.status[host.id]?.problem == .certificateChanged {
            rows.value(StatsText.presented, StatsText.compare, presented.map(LinkFormat.fingerprint) ?? "—", noteColor: Theme.warnText)
            if let presented {
                rows.value(StatsText.trust, "", "", color: Theme.accentLight) {
                    store.trust(fingerprint: presented, for: host)
                }
            }
        }
    }

    // MARK: - Station

    @ViewBuilder private var stationRows: some View {
        let station = info?.station
        let online = store.status[host.id]?.online == true
        let task = store.stationTasks[host.id]
        let working = task?.working == true
        rows.value(info?.stationVersion.map(StationText.version) ?? "KyberNow Station", versionNote.text, "", noteColor: versionNote.warn ? Theme.warnText : Theme.muted)
        if let task { taskRow(task) }
        rows.value(StationText.openAdmin, StationText.adminHelp, "") {
            if let url = ConsoleClient(host: host.address, port: host.consolePort, username: host.username, password: "", fingerprint: nil).adminURL {
                openURL(url)
            }
        }
        if store.viaRelay(host) {
            // Par le relais, pas d'action lourde : Admin refuse le redémarrage et la mise à jour
            // (docs/relais.md) ; la ligne reste, sans action, avec la raison.
            rows.value(StationText.restart, StationText.viaRelay, "", color: Theme.text2)
        } else if station != nil {
            rows.value(StationText.restart, StationText.restartHelp, "") { store.restartStation(host) }
                .disabled(!online || working)
                .opacity(!online || working ? 0.5 : 1)
            if let station, station.updateAvailable, station.installable, !station.requiresSetup, let v = station.updateVersion {
                rows.value(StationText.update, StationText.updateHelp(v), "", color: Theme.accentLight) { store.updateStation(host) }
                    .disabled(!online || working || station.installing)
                    .opacity(!online || working || station.installing ? 0.5 : 1)
            }
        }
        let hasPassword = store.password(for: host) != nil
        rows.value(StationText.account,
                   store.ownDevice(host) ? StationText.ownAccess
                       : hasPassword ? (DeviceDisplay.isPad ? String(localized: "Mot de passe dans le Trousseau de cet iPad.") : String(localized: "Mot de passe dans le Trousseau de cet iPhone."))
                       : String(localized: "Mot de passe introuvable : retirez puis ajoutez de nouveau l'ordinateur."),
                   host.username, noteColor: hasPassword ? Theme.muted : Theme.warnText)
        if store.status[host.id]?.problem == .passwordRefused { passwordRow }
        rows.value(StationText.remove, "", "", color: Theme.dangerText) {
            confirmRemove = true
        }
    }

    /// « À jour », « Version 0.5.1 disponible », l'installateur, ou la Station trop ancienne.
    private var versionNote: (text: String, warn: Bool) {
        guard let info else { return (LinkText.status(store.status[host.id]?.online, away: store.status[host.id]?.away == true), false) }
        guard let station = info.station else { return (StationText.tooOld, true) }
        if station.updateAvailable, let v = station.updateVersion {
            return station.requiresSetup ? (StationText.requiresSetup(v), false) : (StationText.available(v), false)
        }
        return (StationText.upToDate, false)
    }

    /// Redémarrage ou mise à jour : en cours, fait, ou refusé.
    private func taskRow(_ task: StationTask) -> some View {
        let (text, done, warn): (String, Bool, Bool) = switch task {
        case .restarting: (StationText.restarting, false, false)
        case .updating: (StationText.updating, false, false)
        case let .done(message): (message, true, false)
        case let .failed(message): (message, false, true)
        }
        return rows.container {
            HStack(spacing: 12) {
                Group {
                    if task.working {
                        HomeSpinner(size: 18).foregroundStyle(Theme.accentLight)
                    } else {
                        Icon(done ? "check" : "warning", size: 18, bold: done).foregroundStyle(done ? Theme.ok : Theme.warn)
                    }
                }
                .frame(width: 22)
                Text(text)
                    .font(Theme.sans(wideRows ? 15 : 14, .semibold))
                    .foregroundStyle(warn ? Theme.warnText : Theme.text)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .accessibilityElement(children: .combine)
    }

    /// « Mot de passe refusé » : le nouveau, gardé dans le Trousseau puis vérifié.
    private var passwordRow: some View {
        rows.container {
            VStack(alignment: .leading, spacing: 10) {
                rows.label(StationText.newPassword, StationText.passwordHelp)
                HStack(spacing: 8) {
                    SecureField(StationText.password, text: $newPassword)
                        .textContentType(.password)
                        .font(Theme.sans(15))
                        .foregroundStyle(Theme.text)
                        .padding(.horizontal, 12)
                        .frame(minHeight: 44)
                        .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Color.white.opacity(0.08)))
                        .submitLabel(.done)
                        .onSubmit(savePassword)
                    Button(StationText.save, action: savePassword)
                        .buttonStyle(GlassButton(height: 44, fontSize: 15))
                        .disabled(newPassword.isEmpty)
                }
            }
        }
    }

    private func savePassword() {
        guard !newPassword.isEmpty else { return }
        store.updatePassword(newPassword, for: host)
        newPassword = ""
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
