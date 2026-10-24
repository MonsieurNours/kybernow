// Réglages d'un ordinateur (maquette « 7 · Réglages ») : image (au plus 4K à
// 60 Hz, la limite de l'Apple TV), son, manettes et télécommande, réseau,
// compte et iCloud. Partir d'un profil ; toucher à l'image ou au réseau le
// rend « Personnalisé ».

import KyberKit
import Network
import SwiftUI

struct SettingsView: View {
    let hostID: UUID
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @ObservedObject private var prefs = Preferences.shared
    @State private var section = 0
    @State private var confirmRemove = false
    @State private var link = "—"
    /// Langue choisie ici ; celle du lancement reste affichée jusqu'à la relance.
    @State private var language = LanguageChoice.current
    private static let launchLanguage = LanguageChoice.current
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, section(Int), profile(Int), row(String) }

    private var host: Host? { store.host(hostID) }
    private var hostName: String { host?.name ?? "" }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            if let host {
                content(host)
            }
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, .section(0))
        .onExitCommand { app.screen = .home }
        .onAppear(perform: watchLink)
        .alert("Retirer \(hostName) de cette Apple TV ?", isPresented: $confirmRemove) {
            Button("Retirer", role: .destructive) {
                if let host { store.remove(host) }
                app.selected = store.ordered.first?.id
                app.screen = .home
            }
            Button("Annuler", role: .cancel) {}
        } message: {
            Text("Ses accès sont effacés d'ici. L'ordinateur reste sur vos autres appareils.")
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
                Text("Profil de \(host.name)").font(Theme.sans(20)).foregroundStyle(Theme.muted)
                HStack(spacing: 6) {
                    ForEach(Array((Profile.allCases.map(Optional.some) + [nil]).enumerated()), id: \.offset) { i, p in
                        Button(p?.label ?? String(localized: "Personnalisé")) { if let p { edit { $0.apply(p) } } }
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
                .padding(.leading, 96).padding(.top, 170)

            detail(host)
                .padding(.horizontal, 52).padding(.vertical, 44)
                .frame(width: 1920 - 700 - 96, height: 1080 - 170 - 96, alignment: .topLeading)
                .glass(30)
                .padding(.leading, 700).padding(.top, 170)
        }
    }

    // MARK: Sections

    private func sections(_ host: Host) -> some View {
        let s = host.settings
        let mode = TVDisplay.mode(for: s)
        let route = AudioRoute.current
        let menu = [prefs.menuBack ? String(localized: "Retour") : nil, prefs.menuCombo ? String(localized: "combo") : nil].compactMap { $0 }.joined(separator: " + ")
        let items = [
            (String(localized: "Image"), "\(TVDisplay.label(mode.width, mode.height)) · \(mode.refresh) Hz · \(s.bitrateMbps) Mbit/s"),
            (String(localized: "Son"), s.audio == .off ? String(localized: "Sur l'ordinateur") : "\(route.channelsLabel) · \(route.name)"),
            (String(localized: "Manettes et télécommande"), prefs.rumble ? String(localized: "\(menu) · vibrations") : menu),
            (String(localized: "Réseau"), "\(s.transport.label) · \(link)"),
            (String(localized: "Compte et iCloud"), prefs.icloudSync ? String(localized: "\(host.username) · synchronisé") : host.username),
        ]
        return VStack(alignment: .leading, spacing: 12) {
            Text("Réglages").font(Theme.display(64, 660)).tracking(-64 * 0.035).padding(.bottom, 22)
            ForEach(Array(items.enumerated()), id: \.offset) { i, item in
                Button { section = i } label: {
                    HStack {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(item.0).font(Theme.sans(26, .semibold))
                            Text(item.1).font(Theme.sans(18)).foregroundStyle(Theme.muted).lineLimit(1)
                        }
                        Spacer()
                        Icon("arrow-right", size: 22, bold: true).foregroundStyle(Theme.faint)
                    }
                    .padding(.horizontal, 26)
                    .frame(width: 520, height: 100)
                }
                .buttonStyle(GlassButton(radius: 20, selected: section == i, scale: 1.03, fill: section == i ? Color.white.opacity(0.1) : Color.clear))
                .focused($focus, equals: .section(i))
            }
        }
        .focusSection()
        .onChange(of: focus) { _, new in
            if case let .section(i)? = new { section = i }
        }
    }

    // MARK: Détail

    @ViewBuilder private func detail(_ host: Host) -> some View {
        let s = host.settings
        VStack(alignment: .leading, spacing: 14) {
            switch section {
            case 0:
                Theme.eyebrow(String(localized: "Image · au plus 4K à 60 Hz, la limite de l'Apple TV")).padding(.bottom, 8)
                imageRows(s)
            case 1:
                Theme.eyebrow(String(localized: "Son · sortie détectée : \(AudioRoute.current.name), \(AudioRoute.current.channelsLabel)")).padding(.bottom, 8)
                soundRows(s)
            case 2:
                Theme.eyebrow(String(localized: "Manettes et télécommande")).padding(.bottom, 8)
                inputRows(s)
            case 3:
                Theme.eyebrow(String(localized: "Réseau · \(link)")).padding(.bottom, 8)
                networkRows(s, host: host)
            default:
                Theme.eyebrow(String(localized: "Compte et iCloud · \(host.username)")).padding(.bottom, 8)
                accountRows(s, host: host)
            }
        }
        .focusSection()
        .id(section)
        .transition(.opacity.combined(with: .offset(x: 24)))
        .animation(Theme.easeOut, value: section)
    }

    @ViewBuilder private func imageRows(_ s: ConnectionSettings) -> some View {
        let tv = TVDisplay.native
        let mode = TVDisplay.mode(for: s)
        let res = s.display == .custom ? TVDisplay.label(mode.width, mode.height) : "Auto"
        seg("res", String(localized: "Définition"), String(localized: "Auto : celle de ce téléviseur (\(tv.width) × \(tv.height))"), [("Auto", String(localized: "Auto"))] + plain(["4K", "1440p", "1080p"]), res) { v in
            edit { st in
                switch v {
                case "4K": st.display = .custom; st.customWidth = 3840; st.customHeight = 2160
                case "1440p": st.display = .custom; st.customWidth = 2560; st.customHeight = 1440
                case "1080p": st.display = .custom; st.customWidth = 1920; st.customHeight = 1080
                default:
                    if st.customRefresh == tv.refresh { st.display = .match } else { st.display = .custom; st.customWidth = tv.width; st.customHeight = tv.height }
                }
                st.profile = st.profile
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
        rateRow(s)
        seg("codec", String(localized: "Compression"), String(localized: "AV1 : pas de décodage matériel sur cette Apple TV"), plain(["HEVC", "H.264", "AV1"]), s.codec == .h264 ? "H.264" : "HEVC", disabled: ["AV1"] + (DecoderCapabilities.current.hevc ? [] : ["HEVC"])) { v in
            edit { st in
                st.codec = v == "H.264" ? .h264 : .hevc
                st.profile = nil
            }
        }
        toggle("yuv", String(localized: "Texte net (4:4:4)"), ConnectionSettings.decodes444(s.codec) ? String(localized: "Pour le bureau ; testé au lancement sur cette Apple TV") : String(localized: "Cette Apple TV ne le décode pas en matériel"), s.yuv444 && ConnectionSettings.decodes444(s.codec)) {
            edit { st in st.yuv444.toggle(); st.profile = nil }
        }
        .disabled(!ConnectionSettings.decodes444(s.codec))
        toggle("match", String(localized: "Caler le téléviseur sur la session"), String(localized: "Passe le téléviseur en \(mode.refresh) Hz le temps de la session, puis revient"), prefs.matchFrameRate) {
            prefs.matchFrameRate.toggle()
        }
    }

    @ViewBuilder private func soundRows(_ s: ConnectionSettings) -> some View {
        let route = AudioRoute.current
        let channels: [AudioChoice] = [.auto, .stereo, .surround51, .surround71]
        seg("ch", String(localized: "Canaux"), String(localized: "Auto : \(route.channelsLabel) sur cette sortie"), channels.map { ($0.rawValue, $0.label) }, channels.contains(s.audio) ? s.audio.rawValue : AudioChoice.auto.rawValue) { v in
            edit { st in st.audio = AudioChoice(rawValue: v) ?? .auto }
        }
        .disabled(s.audio == .off)
        seg("where", String(localized: "Le son joue"), String(localized: "Sur l'ordinateur : ses haut-parleurs ou son casque"), [("tv", "Apple TV"), ("host", String(localized: "Ordinateur"))], s.audio == .off ? "host" : "tv") { v in
            edit { st in st.audio = v == "host" ? .off : .auto }
        }
        toggle("spatial", String(localized: "Audio spatial dans les AirPods"), String(localized: "Le 5.1 ou le 7.1 du jeu, autour de vous ; sans suivi de la tête"), prefs.spatialAudio) {
            prefs.spatialAudio.toggle()
        }
        toggle("latency", String(localized: "Prévenir d'un son en retard"), String(localized: "AirPods, HomePod, AirPlay : alerte au-delà de 40 ms ajoutées"), prefs.latencyAlert) {
            prefs.latencyAlert.toggle()
        }
        value("delay", String(localized: "Retard mesuré de la sortie"), String(localized: "Mis à jour à chaque changement de sortie"), "\(route.latencyMs) ms")
    }

    @ViewBuilder private func inputRows(_ s: ConnectionSettings) -> some View {
        toggle("back", String(localized: "Menu avec le bouton Retour"), String(localized: "Télécommande : le bouton Retour ouvre le menu de session"), prefs.menuBack) {
            if prefs.menuBack, !prefs.menuCombo { return }
            prefs.menuBack.toggle()
        }
        toggle("combo", String(localized: "Menu avec L1 + R1 + Start + Select"), String(localized: "Comme sur Mac, quand la télécommande est loin"), prefs.menuCombo) {
            if prefs.menuCombo, !prefs.menuBack { return }
            prefs.menuCombo.toggle()
        }
        // En profil Bureau, la session met la télécommande en souris d'office
        // (SessionController) : l'interrupteur le montre, sans se laisser changer.
        let bureau = s.profile == .bureau
        toggle("mouse", String(localized: "Télécommande en souris"),
               bureau ? String(localized: "Toujours active en profil Bureau ; un autre profil la rend réglable") : String(localized: "Pavé : pointeur, clic : clic gauche ; toujours active en profil Bureau"),
               bureau || s.mouse == .desktop) {
            edit { st in st.mouse = st.mouse == .desktop ? .auto : .desktop }
        }
        .disabled(bureau)
        seg("gamepad", String(localized: "Manette vue par l'ordinateur"), s.gamepad.hint, GamepadChoice.allCases.map { ($0.rawValue, $0.label) }, s.gamepad.rawValue) { v in
            edit { st in st.gamepad = GamepadChoice(rawValue: v) ?? .xbox360 }
        }
        toggle("rumble", String(localized: "Vibrations"), String(localized: "Renvoyées par le jeu, sur chaque manette"), prefs.rumble) {
            prefs.rumble.toggle()
        }
        seg("padspeed", String(localized: "Manette en souris : vitesse"), String(localized: "Stick gauche : pointeur, droit : molette · A clic, X clic droit, B milieu · L1 / R1 : Alt + Tab · LT précis, RT rapide"), Preferences.PadMouseSpeed.allCases.map { ($0.rawValue, $0.label) }, prefs.padMouseSpeed.rawValue) { v in
            prefs.padMouseSpeed = Preferences.PadMouseSpeed(rawValue: v) ?? .normal
        }
        seg("guide", String(localized: "Bouton Guide"), String(localized: "Jeu : le bouton Home de la manette va au jeu (Steam, Game Bar) si tvOS le laisse, à vérifier dans Tester les manettes · Appui long : Menu ou Options tenus 0,6 s"), Preferences.GuideChoice.allCases.map { ($0.rawValue, $0.label) }, prefs.guideButton.rawValue) { v in
            prefs.guideButton = Preferences.GuideChoice(rawValue: v) ?? .game
        }
        seg("order", String(localized: "Ordre des joueurs"), String(localized: "Fixe : chaque modèle garde sa place ; deux manettes identiques ne se distinguent pas"), [("power", String(localized: "Allumage")), ("fixed", String(localized: "Fixe"))], prefs.padOrderFixed ? "fixed" : "power") { v in
            prefs.padOrderFixed = v == "fixed"
        }
        toggle("iso", String(localized: "Clavier Apple ISO"), String(localized: "Clavier Apple européen (touche @# ou §± sous Échap) : la touche sous Échap et celle près de Maj retrouvent leur place sur l'ordinateur"), prefs.appleISOKeyboard) {
            prefs.appleISOKeyboard.toggle()
        }
        value("test", String(localized: "Tester les manettes"), padsSummary, GamepadBridge.shared.pads.map(\.label).joined(separator: " · ")) {
            app.screen = .gamepads
        }
    }

    private var padsSummary: String {
        let n = GamepadBridge.shared.pads.count
        let waiting = GamepadBridge.shared.pads.filter(\.waiting).count
        var text = n == 0 ? String(localized: "Aucune manette") : n == 1 ? String(localized: "1 manette") : String(localized: "\(n) manettes")
        if waiting > 0 { text += " " + String(localized: "(\(waiting) en attente : quatre au plus sur l'ordinateur)") }
        if GamepadBridge.shared.remoteConnected { text += ", " + String(localized: "1 télécommande") }
        return text
    }

    @ViewBuilder private func networkRows(_ s: ConnectionSettings, host: Host) -> some View {
        seg("transport", String(localized: "Transport"), String(localized: "Rapide : image perdue plutôt qu'attendue"), Transport.allCases.map { ($0.rawValue, $0.label) }, s.transport.rawValue) { v in
            edit { st in st.transport = Transport(rawValue: v) ?? .fast; st.profile = nil }
        }
        toggle("reconnect", String(localized: "Se reconnecter tout seul"), String(localized: "Après une coupure, la session reprend où elle était"), s.autoReconnect) {
            edit { st in st.autoReconnect.toggle() }
        }
        value("link", String(localized: "Qualité du lien"), String(localized: "Pour la 4K60, l'Ethernet de l'Apple TV est plus stable"),
              host.lastLatencyMs.map { "\(link) · \(HomeView.ms($0)) ms" } ?? link)
    }

    @ViewBuilder private func accountRows(_ s: ConnectionSettings, host: Host) -> some View {
        toggle("icloud", String(localized: "Synchroniser avec iCloud"), String(localized: "Ordinateurs et accès suivent votre Mac et votre compte"), prefs.icloudSync) {
            prefs.icloudSync.toggle()
        }
        // Langue de l'app (docs/i18n.md) : prise en compte à la relance.
        seg("language", String(localized: "Langue"),
            language == Self.launchLanguage ? String(localized: "Auto : la langue de l'Apple TV ; pour KyberNow Link seulement") : String(localized: "Relancez KyberNow Link pour changer de langue."),
            LanguageChoice.allCases.map { ($0.rawValue, $0.label) }, language.rawValue) { v in
            language = LanguageChoice(rawValue: v) ?? .auto
            LanguageChoice.current = language
        }
        toggle("keep", String(localized: "Garder la dernière image"), String(localized: "Le fond de l'accueil, sur cette Apple TV seulement"), s.keepLastImage) {
            edit { st in st.keepLastImage.toggle() }
            if let h = store.host(hostID), !h.settings.keepLastImage { store.imagery.forgetLast(h) }
        }
        value("remove", String(localized: "Retirer \(host.name) de cette Apple TV"), String(localized: "Ses accès sont effacés d'ici"), "", danger: true) {
            confirmRemove = true
        }
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
            Text(note).font(Theme.sans(17)).foregroundStyle(Theme.muted).lineLimit(2)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Options identiques à afficher telles quelles (définitions, fréquences, codecs).
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
            .focusSection()
        }
        .padding(.horizontal, 24).padding(.vertical, 10)
        .frame(minHeight: 84)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
    }

    private func toggle(_ id: String, _ title: String, _ note: String, _ on: Bool, flip: @escaping () -> Void) -> some View {
        Button(action: flip) {
            HStack(spacing: 24) {
                label(title, note)
                Toggle2(on: on)
            }
            .padding(.horizontal, 24).padding(.vertical, 10)
            .frame(minHeight: 84)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .row(id))
        .accessibilityValue(on ? "Activé" : "Désactivé")
    }

    private func value(_ id: String, _ title: String, _ note: String, _ value: String, danger: Bool = false, action: (() -> Void)? = nil) -> some View {
        Button { action?() } label: {
            HStack(spacing: 24) {
                label(title, note, color: danger ? Theme.dangerText : Theme.text)
                Text(value).font(Theme.mono(19)).foregroundStyle(Theme.text2)
                if action != nil {
                    Icon("arrow-right", size: 22, bold: true).foregroundStyle(Theme.faint)
                }
            }
            .padding(.horizontal, 24).padding(.vertical, 10)
            .frame(minHeight: 84)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .row(id))
    }

    private func rateRow(_ s: ConnectionSettings) -> some View {
        let rate = s.bitrateMbps
        let warn = rate > 150
        return HStack(spacing: 24) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .firstTextBaseline, spacing: 14) {
                    Text("Débit").font(Theme.sans(24, .semibold))
                    Text(verbatim: "\(rate) Mbit/s").font(Theme.mono(22)).foregroundStyle(Theme.accentLight)
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
                Button("−") { edit { st in st.bitrateMbps = max(ConnectionSettings.minBitrate, st.bitrateMbps - 10); st.profile = nil } }
                    .buttonStyle(ChoiceButton(selected: false, height: 64, light: true))
                    .focused($focus, equals: .row("rate.less"))
                    .accessibilityLabel("Moins de débit")
                Button("+") { edit { st in st.bitrateMbps = min(ConnectionSettings.maxBitrate, st.bitrateMbps + 10); st.profile = nil } }
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
