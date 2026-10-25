// Réglages d'un ordinateur (maquette « 7 · Réglages », docs/link-ui.md) :
// « Réglages de {nom} », le profil en haut, et les six rubriques communes à
// toutes les apps KyberNow Link : Image (au plus 4K à 60 Hz, la limite de
// l'Apple TV), Son, Commandes, Réseau, Statistiques, Station. Partir d'un
// profil ; toucher au format, au 4:4:4, au débit ou au transport le rend
// « Personnalisé ». Ce qui vaut pour toute l'app (iCloud, langue, test des
// manettes, ce que l'Apple TV sait décoder) est dans les Réglages de l'app.

import CoreImage.CIFilterBuiltins
import KyberKit
import Network
import SwiftUI

struct SettingsView: View {
    let hostID: UUID
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @ObservedObject private var prefs = Preferences.shared
    @State private var section: ComputerSection = .picture
    @State private var confirmRemove = false
    @State private var link = "—"
    @State private var newPassword = ""
    @State private var presented: String?
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, section(ComputerSection), profile(Int), row(String) }

    private var host: Host? { store.host(hostID) }
    private var hostName: String { host?.name ?? "" }
    private var status: HostStatus? { store.status[hostID] }
    private var info: HostInfo? { status?.info }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            if let host {
                content(host)
            }
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, .section(app.settingsSection))
        .onExitCommand {
            // Retour depuis le panneau : la rubrique choisie ; depuis la liste : l'accueil.
            switch focus {
            case .row?, .profile?: focus = .section(section)
            default: app.screen = .home
            }
        }
        .onAppear {
            section = app.settingsSection
            watchLink()
        }
        .alert(StationText.removeQuestion(hostName), isPresented: $confirmRemove) {
            Button(StationText.removeConfirm, role: .destructive) {
                if let host { store.remove(host) }
                app.selected = store.ordered.first?.id
                app.screen = .home
            }
            Button(StationText.cancel, role: .cancel) {}
        }
        .alert(store.stationConfirm?.title ?? "", isPresented: Binding(get: { store.stationConfirm?.hostID == hostID }, set: { if !$0 { store.stationConfirm = nil } }),
               presenting: store.stationConfirm) { c in
            Button(c.confirmLabel, role: .destructive) {
                store.stationConfirm = nil
                guard let host else { return }
                if c.kind == .restart { store.restartStation(host, force: true) } else { store.updateStation(host, force: true) }
            }
            Button(StationText.cancel, role: .cancel) { store.stationConfirm = nil }
        } message: { c in
            Text(c.message)
        }
    }

    private func content(_ host: Host) -> some View {
        ZStack(alignment: .topLeading) {
            Button { app.screen = .home } label: {
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

            HStack(spacing: 16) {
                Text("Profil").font(Theme.sans(20)).foregroundStyle(Theme.muted)
                HStack(spacing: 6) {
                    ForEach(Array((Profile.allCases.map(Optional.some) + [nil]).enumerated()), id: \.offset) { i, p in
                        Button(Profile.label(p)) { if let p { edit { $0.apply(p) } } }
                            .buttonStyle(ChoiceButton(selected: host.settings.profile == p, height: 54))
                            .focused($focus, equals: .profile(i))
                            .disabled(p == nil && host.settings.profile != nil)
                    }
                }
                .padding(6)
                .glass(16)
            }
            .frame(width: 1920 - 192, alignment: .trailing)
            .focusSection()
            .padding(.leading, 96).padding(.top, 56)

            sections(host)
                .frame(width: 520, alignment: .leading)
                .padding(.leading, 96).padding(.top, 150)

            detail(host)
                .frame(width: 1920 - 700 - 96, height: 1080 - 150 - 64, alignment: .topLeading)
                .glass(30)
                .padding(.leading, 700).padding(.top, 150)
        }
    }

    // MARK: Rubriques

    private func sections(_ host: Host) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(LinkText.settingsTitle(host.name))
                .font(Theme.display(48, 660)).tracking(-48 * 0.035)
                .lineLimit(2).minimumScaleFactor(0.7)
                .padding(.bottom, 16)
            VStack(alignment: .leading, spacing: 10) {
                ForEach(ComputerSection.allCases) { sec in
                    Button { section = sec } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(sec.title).font(Theme.sans(26, .semibold))
                                Text(verbatim: subtitle(sec, host)).font(Theme.sans(18)).foregroundStyle(Theme.muted).lineLimit(1)
                            }
                            Spacer()
                            Icon("arrow-right", size: 22, bold: true).foregroundStyle(Theme.faint)
                        }
                        .padding(.horizontal, 26)
                        .frame(width: 520, height: 92)
                    }
                    .buttonStyle(GlassButton(radius: 20, selected: section == sec, scale: 1.03, fill: section == sec ? Color.white.opacity(0.1) : Color.clear))
                    .focused($focus, equals: .section(sec))
                }
            }
            .focusSection()
            // Revenir du panneau (gauche) ramène à la rubrique choisie, quelle que soit la hauteur.
            .defaultFocus($focus, .section(section), priority: .userInitiated)
        }
        .onChange(of: focus) { _, new in
            if case let .section(sec)? = new { section = sec }
        }
    }

    /// Résumé d'une rubrique, sous son titre : simple, sans chiffre technique.
    private func subtitle(_ sec: ComputerSection, _ host: Host) -> String {
        let s = host.settings
        switch sec {
        case .picture:
            switch s.display {
            case .physical:
                return String(localized: "Écrans de l'ordinateur")
            case .custom:
                let mode = TVDisplay.mode(for: s)
                return SectionSummary.picture(TVDisplay.label(mode.width, mode.height), refresh: mode.refresh)
            case .match, .window:
                return SectionSummary.picture(String(localized: "Comme cet écran"), refresh: TVDisplay.mode(for: s).refresh)
            }
        case .sound:
            return s.audio.label
        case .controls:
            switch (prefs.menuBack, prefs.menuCombo) {
            case (true, true): return String(localized: "Menu : Retour et combinaison")
            case (false, true): return String(localized: "Menu : combinaison")
            default: return String(localized: "Menu : Retour")
            }
        case .network:
            return s.transport.label
        case .statistics:
            return SectionSummary.statistics(host)
        case .station:
            if info == nil, status?.online == false { return LinkText.status(false) }
            return SectionSummary.station(info)
        }
    }

    /// Débit de la session pour la définition en cours.
    private func bitrate(_ s: ConnectionSettings) -> Int {
        let m = TVDisplay.mode(for: s)
        return s.bitrate(width: m.width, height: m.height, refresh: m.refresh)
    }

    // MARK: Détail

    @ViewBuilder private func detail(_ host: Host) -> some View {
        let s = host.settings
        // Les lignes défilent sous le focus quand elles ne tiennent pas.
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 12) {
                Theme.eyebrow(caption(host)).padding(.bottom, 8)
                switch section {
                case .picture: pictureRows(s)
                case .sound: soundRows(s)
                case .controls: controlRows(s)
                case .network: networkRows(s, host: host)
                case .statistics: statisticsRows(host)
                case .station: stationRows(host)
                }
            }
            .padding(.horizontal, 52).padding(.vertical, 44)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        // Ce qui dépasse s'efface en bas du panneau, au lieu d'être coupé net.
        .mask(LinearGradient(stops: [.init(color: .black, location: 0), .init(color: .black, location: 0.93), .init(color: .clear, location: 1)],
                             startPoint: .top, endPoint: .bottom))
        // Entrer dans le panneau (droite) mène à sa première ligne, même
        // depuis une rubrique du bas de la liste, plus bas que les lignes.
        .focusSection()
        .defaultFocus($focus, entry(host), priority: .userInitiated)
        .id(section)
        .transition(.opacity.combined(with: .offset(x: 24)))
        .animation(Theme.easeOut, value: section)
    }

    /// La première ligne focalisable de la rubrique.
    private func entry(_ host: Host) -> Target {
        let s = host.settings
        switch section {
        case .picture: return .row("res.match")
        case .sound: return .row("sound.\(AudioChoice.auto.rawValue)")
        case .controls: return s.profile == .bureau ? .row("back") : .row("mouse")
        case .network: return .row("transport.\(Transport.fast.rawValue)")
        case .statistics: return .row("fact.\(StatsText.system)")
        case .station:
            let ready = info?.station != nil && status?.online == true && store.stationTasks[hostID]?.working != true
            return ready ? .row("restart") : .row("fact.\(StationText.account)")
        }
    }

    private func caption(_ host: Host) -> String {
        switch section {
        case .picture: return String(localized: "Image · au plus 4K à 60 Hz, la limite de l'Apple TV")
        case .sound: return String(localized: "Son · sortie : \(AudioRoute.current.name), \(AudioRoute.current.channelsLabel)")
        case .controls: return String(localized: "Commandes · télécommande, manettes, clavier")
        case .network: return String(localized: "Réseau · \(link)")
        case .statistics: return ComputerSection.statistics.title
        case .station: return ComputerSection.station.title
        }
    }

    // MARK: Image

    @ViewBuilder private func pictureRows(_ s: ConnectionSettings) -> some View {
        let tv = TVDisplay.native
        let mode = TVDisplay.mode(for: s)
        let res = s.display == .custom ? TVDisplay.label(mode.width, mode.height) : "match"
        seg("res", String(localized: "Définition"), String(localized: "Comme cet écran : \(LinkFormat.dims(tv.width, tv.height))"),
            [("match", String(localized: "Comme cet écran"))] + plain(["4K", "1440p", "1080p"]), res) { v in
            edit { st in
                switch v {
                case "4K": st.display = .custom; st.customWidth = 3840; st.customHeight = 2160
                case "1440p": st.display = .custom; st.customWidth = 2560; st.customHeight = 1440
                case "1080p": st.display = .custom; st.customWidth = 1920; st.customHeight = 1080
                default:
                    if st.customRefresh == tv.refresh { st.display = .match } else { st.display = .custom; st.customWidth = tv.width; st.customHeight = tv.height }
                }
            }
        }
        seg("hz", String(localized: "Fréquence"), String(localized: "L'écran virtuel de l'ordinateur suit"), plain(["60 Hz", "50 Hz", "30 Hz"]), "\(mode.refresh) Hz") { v in
            edit { st in
                let hz = Int(v.prefix(2)) ?? 60
                st.customRefresh = hz
                if st.display != .custom {
                    if hz == tv.refresh { st.display = .match } else { st.display = .custom; st.customWidth = tv.width; st.customHeight = tv.height }
                }
            }
        }
        .disabled(s.display == .physical)
        toggle("virtual", String(localized: "Écran virtuel"),
               info?.virtualDisplay == false ? String(localized: "L'ordinateur n'a pas d'écran virtuel : ses écrans restent tels quels.") : SettingsText.virtualDisplayHelp,
               s.display != .physical) {
            edit { st in st.display = st.display == .physical ? .match : .physical }
        }
        rateRow(s)
        seg("codec", String(localized: "Format vidéo"), String(localized: "AV1 : cette Apple TV ne le décode pas en matériel"),
            [("hevc", "HEVC"), ("h264", "H.264"), ("av1", "AV1")], s.codec == .h264 ? "h264" : "hevc",
            disabled: ["av1"] + (DecoderCapabilities.current.hevc ? [] : ["hevc"])) { v in
            edit { st in st.codec = v == "h264" ? .h264 : .hevc }
        }
        let can444 = ConnectionSettings.decodes444(s.codec)
        toggle("yuv", String(localized: "Texte net (4:4:4)"),
               can444 ? SettingsText.sharpTextHelp : s.codec == .h264 ? String(localized: "Le 4:4:4 demande le HEVC.") : String(localized: "Cette Apple TV ne le décode pas en matériel : l'image reste en 4:2:0."),
               s.yuv444 && can444) {
            edit { st in st.yuv444.toggle() }
        }
        .disabled(!can444)
        toggle("keep", String(localized: "Garder la dernière image"), String(localized: "Le fond de l'accueil, sur cette Apple TV seulement"), s.keepLastImage) {
            edit { st in st.keepLastImage.toggle() }
            if let h = store.host(hostID), !h.settings.keepLastImage { store.imagery.forgetLast(h) }
        }
        toggle("match", String(localized: "Caler le téléviseur sur la session"), String(localized: "Le téléviseur passe en \(mode.refresh) Hz le temps de la session, puis revient"), prefs.matchFrameRate) {
            prefs.matchFrameRate.toggle()
        }
    }

    private func rateRow(_ s: ConnectionSettings) -> some View {
        let rate = bitrate(s)
        let warn = rate > 150
        return HStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .firstTextBaseline, spacing: 14) {
                    Text("Débit").font(Theme.sans(24, .semibold))
                    Text(verbatim: "\(rate) Mbit/s").font(Theme.mono(22)).foregroundStyle(Theme.accentLight)
                    if s.profile != nil {
                        Text("Auto").font(Theme.sans(18)).foregroundStyle(Theme.faint)
                    }
                }
                GeometryReader { geo in
                    ZStack(alignment: .leading) {
                        HStack(spacing: 0) {
                            Rectangle().fill(Color.white.opacity(0.14)).frame(width: geo.size.width * 0.375)
                            Rectangle().fill(Theme.warn.opacity(0.35))
                        }
                        Rectangle().fill(warn ? Theme.warn : Theme.accent)
                            .frame(width: geo.size.width * min(1, CGFloat(rate) / 400))
                    }
                    .clipShape(Capsule())
                }
                .frame(height: 10)
                Text(warn ? "Au-delà de 150 Mbit/s, l'Apple TV peut perdre des images en 4K60. Gardez-le pour un réseau filaire."
                          : "En Wi-Fi, 50 à 80 Mbit/s suffisent en 4K60 ; en Ethernet, jusqu'à 150.")
                    .font(Theme.sans(17)).foregroundStyle(warn ? Theme.warnText : Theme.muted)
            }
            HStack(spacing: 8) {
                Button { edit { st in st.bitrateMbps = max(ConnectionSettings.minBitrate, rate - 10) } } label: { Text(verbatim: "−") }
                    .buttonStyle(ChoiceButton(selected: false, height: 64, light: true))
                    .focused($focus, equals: .row("rate.less"))
                    .accessibilityLabel("Moins de débit")
                Button { edit { st in st.bitrateMbps = min(ConnectionSettings.maxBitrate, rate + 10) } } label: { Text(verbatim: "+") }
                    .buttonStyle(ChoiceButton(selected: false, height: 64, light: true))
                    .focused($focus, equals: .row("rate.more"))
                    .accessibilityLabel("Plus de débit")
            }
            .focusSection()
        }
        .padding(.horizontal, 24).padding(.vertical, 14)
        .frame(minHeight: 84)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
    }

    // MARK: Son

    @ViewBuilder private func soundRows(_ s: ConnectionSettings) -> some View {
        let route = AudioRoute.current
        seg("sound", String(localized: "Son"), soundNote(s, route), AudioChoice.allCases.map { ($0.rawValue, $0.label) }, s.audio.rawValue) { v in
            edit { st in st.audio = AudioChoice(rawValue: v) ?? .auto }
        }
        toggle("spatial", String(localized: "Audio spatial dans les AirPods"), String(localized: "Le 5.1 ou le 7.1 du jeu, autour de vous ; sans suivi de la tête"), prefs.spatialAudio) {
            prefs.spatialAudio.toggle()
        }
        .disabled(s.audio == .off)
        toggle("latency", String(localized: "Prévenir d'un son en retard"), String(localized: "AirPods, HomePod, AirPlay : alerte au-delà de 40 ms ajoutées"), prefs.latencyAlert) {
            prefs.latencyAlert.toggle()
        }
    }

    private func soundNote(_ s: ConnectionSettings, _ route: AudioRoute) -> String {
        switch s.audio {
        case .auto: return String(localized: "Auto : \(route.channelsLabel) sur cette sortie")
        case .off: return String(localized: "Ses haut-parleurs ou son casque")
        case .stereo: return String(localized: "Deux voies, sur toutes les sorties")
        case .surround51, .surround71: return String(localized: "Il faut un ampli qui le porte ; sinon, le son est mixé en stéréo")
        }
    }

    // MARK: Commandes

    @ViewBuilder private func controlRows(_ s: ConnectionSettings) -> some View {
        // En profil Bureau, la session met la télécommande en souris d'office
        // (SessionController) : l'interrupteur le montre, sans se laisser changer.
        let bureau = s.profile == .bureau
        toggle("mouse", String(localized: "Télécommande en souris"),
               bureau ? String(localized: "Toujours active en profil Bureau ; un autre profil la rend réglable") : String(localized: "Pavé : pointeur, clic : clic gauche ; toujours active en profil Bureau"),
               bureau || s.mouse == .desktop) {
            edit { st in st.mouse = st.mouse == .desktop ? .auto : .desktop }
        }
        .disabled(bureau)
        toggle("back", String(localized: "Menu avec le bouton Retour"), String(localized: "Télécommande : le bouton Retour ouvre le menu de session"), prefs.menuBack) {
            if prefs.menuBack, !prefs.menuCombo { return }
            prefs.menuBack.toggle()
        }
        toggle("combo", String(localized: "Menu avec L1 + R1 + Start + Select"), String(localized: "À la manette, quand la télécommande est loin"), prefs.menuCombo) {
            if prefs.menuCombo, !prefs.menuBack { return }
            prefs.menuCombo.toggle()
        }
        seg("gamepad", String(localized: "Manette vue par l'ordinateur"), s.gamepad.hint, GamepadChoice.allCases.map { ($0.rawValue, $0.label) }, s.gamepad.rawValue) { v in
            edit { st in st.gamepad = GamepadChoice(rawValue: v) ?? .xbox360 }
        }
        toggle("rumble", String(localized: "Vibrations"), String(localized: "Renvoyées par le jeu, sur chaque manette"), prefs.rumble) {
            prefs.rumble.toggle()
        }
        seg("padspeed", String(localized: "Manette en souris : vitesse"), String(localized: "Stick gauche : pointeur, droit : molette · A clic, X clic droit, B milieu · L1 / R1 : Alt + Tab · LT précis, RT rapide"), Preferences.PadMouseSpeed.allCases.map { ($0.rawValue, $0.label) }, prefs.padMouseSpeed.rawValue) { v in
            prefs.padMouseSpeed = Preferences.PadMouseSpeed(rawValue: v) ?? .normal
        }
        seg("guide", String(localized: "Bouton Guide"), String(localized: "Jeu : le bouton Home de la manette va au jeu (Steam, Game Bar) si tvOS le laisse ; à vérifier dans le test des manettes · Appui long : Menu ou Options tenus 0,6 s"), Preferences.GuideChoice.allCases.map { ($0.rawValue, $0.label) }, prefs.guideButton.rawValue) { v in
            prefs.guideButton = Preferences.GuideChoice(rawValue: v) ?? .game
        }
        seg("order", String(localized: "Ordre des joueurs"), String(localized: "Fixe : chaque modèle garde sa place ; deux manettes identiques ne se distinguent pas"), [("power", String(localized: "Allumage")), ("fixed", String(localized: "Fixe"))], prefs.padOrderFixed ? "fixed" : "power") { v in
            prefs.padOrderFixed = v == "fixed"
        }
        toggle("iso", String(localized: "Clavier Apple ISO"), String(localized: "Clavier Apple européen (touche @# ou §± sous Échap) : la touche sous Échap et celle près de Maj retrouvent leur place sur l'ordinateur"), prefs.appleISOKeyboard) {
            prefs.appleISOKeyboard.toggle()
        }
    }

    // MARK: Réseau

    /// « Réveil par le réseau » : « Prêt » et l'adresse MAC apprise de la
    /// Station, sinon de quoi l'apprendre ; l'aide dit qu'il faut aussi
    /// l'autoriser sur l'ordinateur.
    private func wakeRow(_ host: Host) -> some View {
        let mac = SettingsText.wakeAddress(host)
        let note = mac == nil ? SettingsText.wakeNone + "\n" + SettingsText.wakeHelp : SettingsText.wakeHelp
        return value("wake", SettingsText.wakeTitle, note, mac.map { "\(SettingsText.wakeReady) · \($0)" } ?? "")
    }

    @ViewBuilder private func networkRows(_ s: ConnectionSettings, host: Host) -> some View {
        seg("transport", String(localized: "Transport de l'image"), Transport.help, Transport.allCases.map { ($0.rawValue, $0.label) }, s.transport.rawValue) { v in
            edit { st in st.transport = Transport(rawValue: v) ?? .fast }
        }
        toggle("reconnect", String(localized: "Reconnexion automatique"), String(localized: "Après une coupure, la session reprend où elle était"), s.autoReconnect) {
            edit { st in st.autoReconnect.toggle() }
        }
        wakeRow(host)
        value("link", String(localized: "Lien de cette Apple TV"), String(localized: "Pour la 4K à 60 Hz, l'Ethernet de l'Apple TV est plus stable"), link)
    }

    // MARK: Statistiques

    @ViewBuilder private func statisticsRows(_ host: Host) -> some View {
        LastSessionCard(summary: LastSessionSummary(host))
        Theme.eyebrow(StatsText.thisComputer).padding(.top, 18).padding(.bottom, 4)
        fact(StatsText.system, info?.os ?? "—")
        fact(StatsText.graphics, info?.gpu ?? "—")
        fact(StatsText.address, "\(host.address):\(String(host.consolePort))")
        fact(StatsText.fingerprint, host.consoleFingerprint.map(LinkFormat.fingerprint) ?? "—")
        if status?.problem == .certificateChanged {
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    Icon("warning", size: 26).foregroundStyle(Theme.warn)
                    Text(LinkText.problem(.certificateChanged)).font(Theme.sans(21)).foregroundStyle(Theme.warnText)
                }
                if let presented {
                    fact(StatsText.presented, LinkFormat.fingerprint(presented))
                    Text(StatsText.compare).font(Theme.sans(18)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                    Button(StatsText.trust) { store.trust(fingerprint: presented, for: host) }
                        .buttonStyle(PrimaryButton(height: 64, fontSize: 22))
                        .focused($focus, equals: .row("trust"))
                }
            }
            .padding(.top, 10)
            .task { presented = await store.presentedFingerprint(of: host) }
        }
    }

    /// Ligne d'information : sans action, mais focalisable, pour que la
    /// télécommande entre dans le panneau et le fasse défiler.
    private func fact(_ label: String, _ value: String) -> some View {
        Button {} label: {
            HStack(alignment: .firstTextBaseline, spacing: 24) {
                Text(label).font(Theme.sans(21)).foregroundStyle(Theme.muted)
                Spacer(minLength: 12)
                Text(verbatim: value).font(Theme.mono(20)).foregroundStyle(Theme.text).lineLimit(1).minimumScaleFactor(0.7)
            }
            .padding(.horizontal, 24)
            .frame(minHeight: 56)
        }
        .buttonStyle(GlassButton(radius: 14, scale: 1.02, fill: Color.white.opacity(0.03)))
        .focused($focus, equals: .row("fact.\(label)"))
    }

    // MARK: Station

    @ViewBuilder private func stationRows(_ host: Host) -> some View {
        let station = info?.station
        let task = store.stationTasks[hostID]
        let working = task?.working == true
        HStack(alignment: .top, spacing: 36) {
            VStack(alignment: .leading, spacing: 14) {
                // La version, puis ce qui se passe.
                VStack(alignment: .leading, spacing: 6) {
                    Text(verbatim: info?.stationVersion.map(StationText.version) ?? "KyberNow Station")
                        .font(Theme.display(36, 640)).tracking(-0.8)
                    if let station, !working {
                        if station.updateAvailable, let v = station.updateVersion {
                            Text(StationText.available(v)).font(Theme.sans(21, .semibold)).foregroundStyle(Theme.accentLight)
                        } else {
                            Text(StationText.upToDate).font(Theme.sans(21)).foregroundStyle(Theme.muted)
                        }
                    } else if status?.online == false {
                        Text(LinkText.status(false)).font(Theme.sans(21)).foregroundStyle(Theme.muted)
                    }
                }
                .padding(.bottom, 6)
                if let task { taskLine(task) }
                if station == nil, info != nil {
                    // Station 0.4.9 ou plus ancienne : ni redémarrage ni mise à jour d'ici.
                    Text(StationText.tooOld).font(Theme.sans(20)).foregroundStyle(Theme.warnText)
                        .fixedSize(horizontal: false, vertical: true)
                } else if station != nil {
                    action("restart", StationText.restart, StationText.restartHelp, icon: "arrows-clockwise") {
                        store.restartStation(host)
                    }
                    .disabled(working || status?.online != true)
                    .opacity(working || status?.online != true ? 0.45 : 1)
                    if let station, station.updateAvailable, let v = station.updateVersion {
                        if station.requiresSetup {
                            Text(StationText.requiresSetup(v)).font(Theme.sans(20)).foregroundStyle(Theme.muted)
                                .fixedSize(horizontal: false, vertical: true)
                        } else if station.installable {
                            action("update", StationText.update, StationText.updateHelp(v), icon: "arrow-right") {
                                store.updateStation(host)
                            }
                            .disabled(working || station.installing || status?.online != true)
                            .opacity(working || station.installing || status?.online != true ? 0.45 : 1)
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            adminCode(host)
        }
        fact(StationText.account, host.username)
            .padding(.top, 22)
        if store.ownDevice(host) {
            Text(StationText.ownAccess).font(Theme.sans(18)).foregroundStyle(Theme.muted).padding(.horizontal, 24)
        }
        if status?.problem == .passwordRefused {
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    Icon("warning", size: 26).foregroundStyle(Theme.warn)
                    Text(LinkText.problem(.passwordRefused)).font(Theme.sans(21)).foregroundStyle(Theme.warnText)
                }
                HStack(spacing: 20) {
                    SecureField(StationText.newPassword, text: $newPassword)
                        .font(Theme.sans(24))
                        .frame(width: 520)
                        .focused($focus, equals: .row("password"))
                    Button(StationText.save) {
                        store.updatePassword(newPassword, for: host)
                        newPassword = ""
                    }
                    .buttonStyle(PrimaryButton(height: 64, fontSize: 22))
                    .disabled(newPassword.isEmpty)
                }
                Text(StationText.passwordHelp).font(Theme.sans(18)).foregroundStyle(Theme.muted)
            }
            .padding(.top, 10)
        }
        value("remove", StationText.remove, String(localized: "Ses accès sont effacés de cette Apple TV."), "", danger: true) {
            confirmRemove = true
        }
        .padding(.top, 12)
    }

    /// Ce que fait la Station en ce moment, ou ce qu'elle vient de faire.
    private func taskLine(_ task: StationTask) -> some View {
        HStack(spacing: 14) {
            switch task {
            case .restarting, .updating:
                ProgressView().scaleEffect(0.8)
                Text(task.isRestart ? StationText.restarting : StationText.updating).font(Theme.sans(22, .semibold))
            case let .done(message):
                Icon("check", size: 26, bold: true).foregroundStyle(Theme.ok)
                Text(message).font(Theme.sans(22, .semibold))
            case let .failed(message):
                Icon("warning", size: 26).foregroundStyle(Theme.warn)
                Text(message).font(Theme.sans(20)).foregroundStyle(Theme.warnText).fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(.bottom, 4)
    }

    /// QR code d'Admin : on l'ouvre sur son téléphone (le navigateur demande le mot de passe d'Admin).
    private func adminCode(_ host: Host) -> some View {
        let url = adminURL(host)
        return VStack(spacing: 16) {
            if let qr = Self.qr(url) {
                Image(uiImage: qr)
                    .interpolation(.none)
                    .resizable()
                    .frame(width: 216, height: 216)
                    .padding(20)
                    .background(RoundedRectangle(cornerRadius: 20).fill(Theme.text))
                    .accessibilityLabel(Text(StationText.openAdmin))
            }
            Text(StationText.adminScan)
                .font(Theme.sans(19)).foregroundStyle(Theme.text2)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            Text(verbatim: url).font(Theme.mono(16)).foregroundStyle(Theme.muted)
                .lineLimit(1).minimumScaleFactor(0.7)
        }
        .frame(width: 300)
    }

    private func adminURL(_ host: Host) -> String {
        if let url = store.console(for: host)?.adminURL { return url.absoluteString }
        let address = host.address.contains(":") ? "[\(host.address)]" : host.address
        return "https://\(address):\(String(host.consolePort))/"
    }

    private static func qr(_ text: String) -> UIImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(text.utf8)
        filter.correctionLevel = "M"
        guard let out = filter.outputImage, let cg = CIContext().createCGImage(out, from: out.extent) else { return nil }
        return UIImage(cgImage: cg)
    }

    /// Bouton d'action de la Station : titre, aide, icône.
    private func action(_ id: String, _ title: String, _ note: String, icon: String, run: @escaping () -> Void) -> some View {
        Button(action: run) {
            HStack(spacing: 22) {
                Icon(icon, size: 28).foregroundStyle(Theme.accent)
                label(title, note)
            }
            .padding(.horizontal, 24).padding(.vertical, 10)
            .frame(minHeight: 80)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .row(id))
    }

    // MARK: Lignes

    private func edit(_ change: (inout ConnectionSettings) -> Void) {
        guard var h = host else { return }
        let before = h.settings.profile
        change(&h.settings)
        // Toucher à l'image ou au réseau rend le profil « Personnalisé ».
        if h.settings.profile == before, h.settings != store.host(hostID)?.settings {
            var probe = h.settings
            if let p = before { probe.apply(p) }
            if probe.yuv444 != h.settings.yuv444 || probe.bitrateMbps != h.settings.bitrateMbps
                || probe.transport != h.settings.transport || probe.codec != h.settings.codec {
                h.settings.profile = nil
            }
        }
        store.update(h)
    }

    private func label(_ title: String, _ note: String, color: Color = Theme.text) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(Theme.sans(24, .semibold)).foregroundStyle(color)
            Text(note).font(Theme.sans(17)).foregroundStyle(Theme.muted).lineLimit(3)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Options identiques à afficher telles quelles (définitions, fréquences).
    private func plain(_ options: [String]) -> [(key: String, label: String)] {
        options.map { ($0, $0) }
    }

    /// `options` : clé (comparée à `current`, `disabled`, rendue à `pick`) et texte affiché.
    private func seg(_ id: String, _ title: String, _ note: String, _ options: [(key: String, label: String)], _ current: String, disabled: [String] = [], pick: @escaping (String) -> Void) -> some View {
        HStack(spacing: 24) {
            label(title, note)
            HStack(spacing: 8) {
                ForEach(options, id: \.key) { o in
                    Button { pick(o.key) } label: {
                        Text(o.label).strikethrough(disabled.contains(o.key))
                    }
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
                label(title, note)
                Toggle2(on: on)
            }
            .padding(.horizontal, 24).padding(.vertical, 10)
            .frame(minHeight: 80)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .row(id))
        .accessibilityValue(on ? Text("Activé") : Text("Désactivé"))
    }

    private func value(_ id: String, _ title: String, _ note: String, _ value: String, danger: Bool = false, action: (() -> Void)? = nil) -> some View {
        Button { action?() } label: {
            HStack(spacing: 24) {
                label(title, note, color: danger ? Theme.dangerText : Theme.text)
                Text(verbatim: value).font(Theme.mono(19)).foregroundStyle(Theme.text2)
                if action != nil {
                    Icon("arrow-right", size: 22, bold: true).foregroundStyle(Theme.faint)
                }
            }
            .padding(.horizontal, 24).padding(.vertical, 10)
            .frame(minHeight: 80)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .row(id))
    }

    private func watchLink() {
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { path in
            let kind = path.usesInterfaceType(.wiredEthernet) ? "Ethernet" : path.usesInterfaceType(.wifi) ? "Wi-Fi" : String(localized: "Réseau")
            DispatchQueue.main.async {
                link = kind
                monitor.cancel()
            }
        }
        monitor.start(queue: .global())
    }
}

private extension StationTask {
    var isRestart: Bool {
        if case .restarting = self { return true }
        return false
    }
}
