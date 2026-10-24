// Accueil (maquettes iOS, planche 1 : « Accueil iPhone » et « Accueil
// iPad ») : l'accueil du Mac et la scène de l'Apple TV, au doigt. La dernière
// image de l'ordinateur choisi occupe tout l'écran ; dessus, son nom, son
// état, « Se connecter » et le profil ; en bas, la rangée « Vos ordinateurs »
// qui défile. Toucher une carte met son ordinateur en scène, un appui long
// propose ses réglages ou de le retirer ; + et le QR code ouvrent l'ajout.
//
// iPad en pleine largeur : la composition de l'Apple TV, avec la colonne
// « Prêt à jouer » (manettes, son, clavier, dernière session). En largeur
// réduite (Split View), la composition de l'iPhone, et les lignes de la
// colonne passent sous le bouton.
//
// Au lancement, le symbole et le nom arrivent du centre (là où LaunchView
// les a allumés) et filent à leur place en haut à gauche pendant que les
// blocs montent en cascade.

import KyberKit
import SwiftUI

struct HomeView: View {
    @EnvironmentObject private var app: AppModel

    var body: some View {
        // Les modèles observés directement : l'accueil suit la découverte,
        // iCloud, les images et les manettes, quels que soient les objets
        // d'environnement posés plus haut.
        HomeScreen(app: app, store: app.store, discovery: app.discovery, cloud: app.cloud,
                   imagery: app.store.imagery, pads: GamepadBridge.shared)
            .environmentObject(app.store.imagery)
    }

    // MARK: Textes partagés (réglages, mesures)

    /// Virgule décimale en français, point en anglais.
    static func ms(_ v: Double) -> String {
        let text = String(format: "%.1f", v)
        return LanguageChoice.effective == "fr" ? text.replacingOccurrences(of: ".", with: ",") : text
    }

    /// « aujourd'hui, 10 h 12 », « hier, 21 h 40 », « lundi, 18 h 05 », « il y a 12 jours »
    /// (en anglais : “today, 10:12 AM”, “Monday, 6:05 PM”).
    static func when(_ date: Date) -> String {
        let cal = Calendar.current
        let fr = LanguageChoice.effective == "fr"
        let locale = Locale(identifier: fr ? "fr_FR" : "en_US")
        let hm = fr
            ? date.formatted(.dateTime.hour(.defaultDigits(amPM: .omitted)).minute(.twoDigits).locale(locale))
                .replacingOccurrences(of: ":", with: " h ")
            : date.formatted(.dateTime.hour().minute(.twoDigits).locale(locale))
        if cal.isDateInToday(date) { return String(localized: "aujourd'hui, \(hm)") }
        if cal.isDateInYesterday(date) { return String(localized: "hier, \(hm)") }
        let days = cal.dateComponents([.day], from: cal.startOfDay(for: date), to: cal.startOfDay(for: Date())).day ?? 0
        if days < 7 { return "\(date.formatted(.dateTime.weekday(.wide).locale(locale))), \(hm)" }
        return ago(date)
    }

    /// « aujourd'hui », « hier », « il y a 3 jours » ; en court, « il y a 3 j ».
    static func ago(_ date: Date, short: Bool = false) -> String {
        let cal = Calendar.current
        let days = cal.dateComponents([.day], from: cal.startOfDay(for: date), to: cal.startOfDay(for: Date())).day ?? 0
        if days < 1 { return String(localized: "aujourd'hui") }
        if days == 1 { return String(localized: "hier") }
        return short ? String(localized: "il y a \(days) j") : String(localized: "il y a \(days) jours")
    }

    /// « 2 h 14 », « 14 min », « 45 s ».
    static func duration(_ seconds: TimeInterval) -> String {
        let s = Int(seconds.rounded())
        if s >= 3600 { return String(localized: "\(s / 3600) h \(String(format: "%02d", (s % 3600) / 60))") }
        if s >= 60 { return String(localized: "\(s / 60) min") }
        return String(localized: "\(s) s")
    }

    static func gpu(_ name: String) -> String {
        name.replacingOccurrences(of: "NVIDIA GeForce ", with: "")
            .replacingOccurrences(of: "NVIDIA ", with: "")
            .replacingOccurrences(of: "AMD Radeon ", with: "RX ")
            .replacingOccurrences(of: "Intel(R) ", with: "")
    }
}

// MARK: - L'écran

struct HomeScreen: View {
    @ObservedObject var app: AppModel
    @ObservedObject var store: HostStore
    @ObservedObject var discovery: Discovery
    @ObservedObject var cloud: CloudSync
    @ObservedObject var imagery: HostImagery
    @ObservedObject var pads: GamepadBridge
    @StateObject private var peripherals = HomePeripherals()
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// 1 : le symbole et le nom sont encore au centre, allumés (fin du
    /// lancement) ; 0 : à leur place dans la barre du haut.
    @State private var flight: CGFloat = 0
    /// Les blocs de l'accueil, montés (après le lancement) ou encore en bas.
    @State private var revealed = true
    /// Le décor (la dernière image), qui arrive en fondu après le lancement.
    @State private var sceneShown = true
    @State private var markFrame: CGRect = .zero
    @State private var wordFrame: CGRect = .zero
    /// Lumière du bouton principal : l'allumage de la charte à l'arrivée, la veille au survol.
    @State private var ctaGlow: NeonLevel = .off
    @State private var hovering = false
    @State private var retrying: UUID?
    @State private var removing: Host?
    /// Adresses des ordinateurs annoncés sur le réseau, trouvées en arrière-plan.
    @State private var resolved: [String: Candidate] = [:]

    private static let space = "home"

    private var host: Host? { app.selectedHost }
    private var isPad: Bool { DeviceDisplay.isPad }
    /// iPad en pleine largeur (ou presque) : la scène de l'Apple TV.
    private var wide: Bool { isPad && sizeClass == .regular }
    private func online(_ h: Host) -> Bool { store.status[h.id]?.online ?? false }

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .topLeading) {
                HomeBackdrop(host: host, online: host.map(online) ?? true, wide: wide)
                    .ignoresSafeArea()
                    .opacity(sceneShown ? 1 : 0)
                if wide { wideLayout(geo.size) } else { compactLayout(geo.size) }
            }
            .frame(width: geo.size.width, height: geo.size.height)
            .overlay(alignment: .topLeading) { flyingBrand(geo.size) }
        }
        .coordinateSpace(.named(Self.space))
        .background(shortcuts)
        .confirmationDialog(removeTitle, isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }), titleVisibility: .visible) {
            Button("Retirer", role: .destructive) { if let h = removing { remove(h) } }
            Button("Annuler", role: .cancel) {}
        } message: {
            Text("Ses accès sont effacés d'ici. L'ordinateur reste sur vos autres appareils.")
        }
        // En données mobiles, « Sessions en données mobiles » coupé : on
        // demande avant d'ouvrir la session (AppModel.connect).
        .alert(Text("Se connecter en données mobiles ?"),
               isPresented: Binding(get: { app.mobileDataAsk != nil }, set: { if !$0 { app.mobileDataAsk = nil } }),
               presenting: app.mobileDataAsk) { id in
            Button("Se connecter") { app.connect(id, mobileDataOK: true) }
            Button("Toujours autoriser") {
                app.prefs.allowCellular = true
                app.connect(id)
            }
            Button("Annuler", role: .cancel) {}
        } message: { _ in
            Text("Une session peut consommer plusieurs gigaoctets par heure sur votre forfait.")
        }
        .onAppear {
            if app.selected == nil { app.selected = store.ordered.first?.id }
            #if DEBUG && targetEnvironment(simulator)
            // Réglages de l'app d'emblée (simulateur) : `-screen appSettings`.
            if UserDefaults.standard.string(forKey: "screen") == "appSettings" { app.sheet = .appSettings }
            #endif
            if app.launched {
                igniteCTA(after: 0.9)
            } else if !reduceMotion {
                // Caché sous le lancement : le symbole attend au centre, les blocs en bas.
                flight = 1
                flying = true
                revealed = false
                sceneShown = false
            }
        }
        .onChange(of: app.launched) { _, launched in
            guard launched else { return }
            withAnimation(HomeCurve.rise(0.6)) { flight = 0; sceneShown = true }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.62) { flying = false }
            revealed = true
            igniteCTA(after: 0.9)
        }
        .task(id: discovery.found.map(\.id)) { await resolveFound() }
    }

    // MARK: Lancement : le symbole et le nom filent à leur place

    private var markSize: CGFloat { wide ? 28 : 22 }
    private var wordSize: CGFloat { wide ? 22 : 17 }

    /// Le symbole et le nom de la barre du haut, dessinés par-dessus tout
    /// pendant le vol : ils partent du centre (la place exacte où LaunchView
    /// les a laissés, allumés) et arrivent sur ceux de la barre, cachés le
    /// temps du vol.
    @ViewBuilder private func flyingBrand(_ size: CGSize) -> some View {
        if flying, markFrame != .zero {
            let center = CGPoint(x: size.width / 2, y: size.height / 2)
            let markScale = LaunchView.markSize / markSize
            let wordScale = LaunchView.nameSize / wordSize
            let wordTop = CGPoint(x: center.x, y: center.y + LaunchView.markSize / 2 + LaunchView.nameGap)
            ZStack(alignment: .topLeading) {
                Mark(size: markSize, level: flight > 0 ? .lit : .off)
                    .scaleEffect(1 + (markScale - 1) * flight)
                    .modifier(LaunchGlow(level: flight > 0 ? .lit : .off))
                    .position(x: markFrame.midX + (center.x - markFrame.midX) * flight,
                              y: markFrame.midY + (center.y - markFrame.midY) * flight)
                // Agrandi depuis son bord haut : le haut du nom reste à 22 pt sous le symbole.
                // L'étiquette « LINK » passe de sa taille au lancement à celle de la barre
                // (taille minimale) : affichée = taille × échelle du vol.
                Wordmark(size: wordSize, tagSize: flightTagSize(wordScale: wordScale))
                    .fixedSize()
                    .scaleEffect(1 + (wordScale - 1) * flight, anchor: .top)
                    .position(x: wordFrame.midX + (wordTop.x - wordFrame.midX) * flight,
                              y: wordFrame.midY + (wordTop.y - wordFrame.minY) * flight)
            }
            .frame(width: size.width, height: size.height, alignment: .topLeading)
            .allowsHitTesting(false)
        }
    }

    /// Taille d'étiquette du nom volant, avant son agrandissement : de celle
    /// du lancement (flight = 1) à celle de la barre (flight = 0).
    private func flightTagSize(wordScale: CGFloat) -> CGFloat {
        let bar = max(wordSize, Wordmark.minTagSize)
        let launch = max(LaunchView.nameSize, Wordmark.minTagSize)
        let shown = bar + (launch - bar) * flight
        return shown / (1 + (wordScale - 1) * flight)
    }

    /// Vrai pendant le lancement et le vol : la barre cache son propre
    /// symbole, que le symbole volant remplace jusqu'à l'arrivée.
    @State private var flying = false

    private var brand: some View {
        HStack(spacing: wide ? 14 : 10) {
            Mark(size: markSize)
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(Self.space)) } action: { markFrame = $0 }
            Wordmark(size: wordSize)
                .fixedSize()
                .onGeometryChange(for: CGRect.self) { $0.frame(in: .named(Self.space)) } action: { wordFrame = $0 }
        }
        .opacity(flying ? 0 : 1)
    }

    // MARK: Barre du haut

    private var topBar: some View {
        HStack(spacing: 10) {
            brand
            Spacer(minLength: 0)
            HStack(spacing: 10) {
                if wide {
                    Button { app.sheet = .addHost(nil) } label: {
                        HStack(spacing: 10) {
                            Image(systemName: "qrcode").font(.system(size: 17, weight: .regular))
                            Text("Scanner un QR code")
                        }
                    }
                    .buttonStyle(HomePillStyle())
                } else {
                    Button { app.sheet = .addHost(nil) } label: {
                        Image(systemName: "qrcode").font(.system(size: 18, weight: .regular))
                    }
                    .buttonStyle(HomeCircleStyle())
                    .accessibilityLabel(Text("Scanner un QR code"))
                }
                Button { app.sheet = .addHost(nil) } label: { Icon("plus", size: 20, bold: true) }
                    .buttonStyle(HomeCircleStyle())
                    .accessibilityLabel(Text("Ajouter un ordinateur"))
                settingsButton
            }
            .homeRise(revealed, 0.1)
        }
        .padding(.leading, wide ? 0 : 4)
    }

    /// Toucher : les réglages de l'ordinateur en scène ; appui long : ceux
    /// de l'ordinateur ou ceux de l'app (valables pour tous).
    private var settingsButton: some View {
        Menu {
            if let host {
                Button { app.sheet = .settings(host.id) } label: {
                    Label { Text("Réglages de \(host.name)") } icon: { Image(systemName: "slider.horizontal.3") }
                }
            }
            Button { app.sheet = .appSettings } label: {
                Label { Text("Réglages de l'app") } icon: { Image(systemName: "gearshape") }
            }
        } label: {
            Icon("sliders-horizontal", size: 20)
                .foregroundStyle(Theme.text)
                .frame(width: 44, height: 44)
                .glass(22)
                .contentShape(Circle())
        } primaryAction: {
            openSettings()
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .accessibilityLabel(host != nil ? Text("Réglages de l'ordinateur") : Text("Réglages de l'app"))
    }

    private func openSettings() {
        if let host { app.sheet = .settings(host.id) } else { app.sheet = .appSettings }
    }

    // MARK: Composition de l'iPhone (et de l'iPad en largeur réduite)

    private func compactLayout(_ size: CGSize) -> some View {
        VStack(spacing: 0) {
            topBar
                .padding(.leading, 16).padding(.trailing, 16)
                .frame(height: 44)
                .zIndex(1)
            ScrollView(.vertical) {
                VStack(alignment: .leading, spacing: 0) {
                    Spacer(minLength: 24)
                    hero(wide: false)
                        .padding(.horizontal, 24)
                    actions(wide: false)
                        .padding(.horizontal, 24)
                        .padding(.top, 18)
                    if isPad, host != nil {
                        // Split View : les lignes de « Prêt à jouer » passent sous le bouton.
                        HomeReadyRows(host: host, pads: pads, peripherals: peripherals, size: 14)
                            .padding(16)
                            .glass(16)
                            .padding(.horizontal, 24)
                            .padding(.top, 16)
                            .homeRise(revealed, 0.48)
                    }
                    // Au-dessus de l'indicateur d'accueil, sans le toucher.
                    shelf(width: size.width, wide: false)
                        .padding(.top, 20)
                        .padding(.bottom, 16)
                }
                .frame(minHeight: size.height - 44, alignment: .bottom)
            }
            .scrollBounceBehavior(.basedOnSize)
            .scrollIndicators(.hidden)
        }
    }

    // MARK: Composition de l'iPad (pleine largeur)

    private func wideLayout(_ size: CGSize) -> some View {
        // La colonne « Prêt à jouer » à droite si la place le permet (paysage),
        // sinon sous le bouton (portrait, deux tiers de l'écran).
        let side = size.width >= 1180
        let margin: CGFloat = size.width >= 1000 ? 72 : 48
        return VStack(spacing: 0) {
            topBar
                .frame(height: 48)
                .padding(.top, 16)
                .padding(.horizontal, margin)
                .zIndex(1)
            ScrollView(.vertical) {
                VStack(alignment: .leading, spacing: 0) {
                    HStack(alignment: .top, spacing: 40) {
                        VStack(alignment: .leading, spacing: 18) {
                            hero(wide: true)
                            actions(wide: true).padding(.top, 18)
                        }
                        .frame(maxWidth: 760, alignment: .leading)
                        Spacer(minLength: 0)
                        if side, host != nil {
                            HomeReadyCard(host: host, pads: pads, peripherals: peripherals)
                                .frame(width: 380)
                                .padding(.top, 40)
                                .homeRise(revealed, 0.38)
                        }
                    }
                    if !side, host != nil {
                        HomeReadyCard(host: host, pads: pads, peripherals: peripherals)
                            .frame(maxWidth: 520)
                            .padding(.top, 32)
                            .homeRise(revealed, 0.48)
                    }
                    Spacer(minLength: 40)
                    shelf(width: size.width - 2 * margin, wide: true)
                        .padding(.bottom, 24)
                }
                .padding(.top, size.height > 900 ? 108 : 48)
                .padding(.horizontal, margin)
                .frame(minHeight: size.height - 64, alignment: .top)
            }
            .scrollBounceBehavior(.basedOnSize)
            .scrollIndicators(.hidden)
        }
    }

    // MARK: L'ordinateur en scène

    @ViewBuilder private func hero(wide: Bool) -> some View {
        let gap: CGFloat = wide ? 18 : 12
        if let host {
            let on = online(host)
            VStack(alignment: .leading, spacing: gap) {
                HStack(spacing: wide ? 10 : 9) {
                    Circle().fill(Theme.accent).frame(width: wide ? 7 : 6, height: wide ? 7 : 6)
                    Text(caption(host).uppercased())
                        .font(Theme.mono(wide ? 12 : 11, .medium))
                        .tracking(wide ? 0.96 : 0.88)
                        .foregroundStyle(Theme.muted)
                        .lineLimit(1)
                }
                .homeRise(revealed, 0.1)
                // Deux lignes au plus, interligne serré comme la maquette ; un nom long rétrécit.
                Text(host.name)
                    .font(Theme.display(wide ? 96 : 54, 680))
                    .tracking(wide ? -96 * 0.045 : -54 * 0.04)
                    .lineSpacing(wide ? -14 : -8)
                    .lineLimit(2)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                    .frame(minHeight: wide ? 172 : 100, alignment: .bottomLeading)
                    .homeRise(revealed, 0.18)
                    .accessibilityAddTraits(.isHeader)
                chips(host, online: on, wide: wide)
                    .homeRise(revealed, 0.28)
                if !wide, !specs(host).isEmpty {
                    Text(specs(host)).font(Theme.mono(11)).foregroundStyle(Theme.muted).lineLimit(1)
                        .homeRise(revealed, 0.28)
                }
                HStack(alignment: .firstTextBaseline, spacing: wide ? 10 : 9) {
                    Icon("monitor", size: wide ? 19 : 17).foregroundStyle(Theme.accent)
                        .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 3 }
                    Text(screenLine(host, online: on))
                        .font(Theme.sans(wide ? 16 : 13.5))
                        .foregroundStyle(Theme.text2)
                        .lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .homeRise(revealed, 0.38)
            }
            .animation(Theme.easeOut, value: on)
        } else {
            VStack(alignment: .leading, spacing: gap) {
                HStack(spacing: 9) {
                    Circle().fill(Theme.accent).frame(width: 6, height: 6)
                    Text("Bienvenue".uppercased()).font(Theme.mono(wide ? 12 : 11, .medium)).tracking(0.9).foregroundStyle(Theme.muted)
                }
                .homeRise(revealed, 0.1)
                Text(isPad ? "Vos ordinateurs, sur cet iPad" : "Vos ordinateurs, sur cet iPhone")
                    .font(Theme.display(wide ? 72 : 44, 680))
                    .tracking(wide ? -72 * 0.04 : -44 * 0.04)
                    .lineSpacing(wide ? -10 : -6)
                    .foregroundStyle(Theme.text)
                    .fixedSize(horizontal: false, vertical: true)
                    .homeRise(revealed, 0.18)
                Text("Ajoutez un ordinateur équipé de KyberNow Station : depuis votre Mac par iCloud, sur ce réseau, par un code à accepter sur l'ordinateur ou par son QR code.")
                    .font(Theme.sans(wide ? 17 : 14))
                    .foregroundStyle(Theme.text2)
                    .lineSpacing(2)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: 620, alignment: .leading)
                    .homeRise(revealed, 0.28)
            }
        }
    }

    @ViewBuilder private func chips(_ host: Host, online on: Bool, wide: Bool) -> some View {
        let fromMac = cloud.hosts.contains { $0.id == host.id }
        HStack(spacing: wide ? 14 : 10) {
            HomeChip(height: wide ? 32 : 30, fontSize: wide ? 14 : 13) {
                HomeStatusDot(online: on, size: wide ? 8 : 7)
                Text(status(host)).lineLimit(1)
            }
            if fromMac {
                HomeChip(height: wide ? 32 : 30, fontSize: wide ? 14 : 13) {
                    Icon("cloud", size: wide ? 16 : 15).foregroundStyle(Theme.accent)
                    Text(wide ? "Ajouté depuis votre Mac" : "Depuis votre Mac").foregroundStyle(Theme.text2).lineLimit(1)
                }
            }
            if wide, !specs(host).isEmpty {
                Text(specs(host)).font(Theme.mono(12.5)).foregroundStyle(Theme.muted).lineLimit(1).minimumScaleFactor(0.8)
            }
        }
    }

    // MARK: Se connecter, puis le profil

    @ViewBuilder private func actions(wide: Bool) -> some View {
        if let host {
            let on = online(host)
            let cta = Button {
                if on { app.connect(host.id) } else { retry(host) }
            } label: {
                HStack(spacing: 12) {
                    Text(on ? "Se connecter" : "Réessayer")
                    if !wide { Spacer(minLength: 0) }
                    if retrying == host.id {
                        HomeSpinner(size: 20)
                    } else {
                        Icon("arrow-right", size: 20, bold: true)
                    }
                }
                .padding(.leading, wide ? 22 : 20)
                .padding(.trailing, wide ? 26 : 18)
                .frame(maxWidth: wide ? nil : .infinity)
            }
            .buttonStyle(HomeCTAStyle(online: on, height: wide ? 58 : 56, fontSize: wide ? 18 : 17,
                                      glow: hovering && on ? max(ctaGlow, .hover) : ctaGlow))
            .keyboardShortcut(.defaultAction)
            .onHover { hovering = $0 }
            .hoverEffect(.highlight)
            .disabled(retrying == host.id)
            .accessibilityHint(on ? Text("Ouvre la session sur \(host.name)") : Text("Cherche l'ordinateur à nouveau"))

            let picker = HomeProfilePicker(selected: host.settings.profile, fill: !wide, height: 44, fontSize: wide ? 15 : 14) { p in
                var h = host
                h.settings.apply(p)
                store.update(h)
            }
            let hint = Text(hint(host.settings.profile, wide: wide))
                .font(Theme.sans(wide ? 14 : 12.5))
                .foregroundStyle(Theme.muted)
                .lineSpacing(2)
                .fixedSize(horizontal: false, vertical: true)
                .id(host.settings.profile)
                .transition(.opacity)

            if wide {
                VStack(alignment: .leading, spacing: 18) {
                    HStack(spacing: 16) {
                        cta
                        picker
                    }
                    hint
                }
                .homeRise(revealed, 0.48)
                .animation(Theme.easeOut, value: host.settings.profile)
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    cta
                    picker
                    hint
                }
                .homeRise(revealed, 0.48)
                .animation(Theme.easeOut, value: host.settings.profile)
            }
        } else {
            Button { app.sheet = .addHost(nil) } label: {
                HStack(spacing: 12) {
                    Text("Ajouter un ordinateur")
                    if !wide { Spacer(minLength: 0) }
                    Icon("plus", size: 20, bold: true)
                }
                .padding(.leading, wide ? 22 : 20)
                .padding(.trailing, wide ? 26 : 18)
                .frame(maxWidth: wide ? nil : .infinity)
            }
            .buttonStyle(HomeCTAStyle(online: true, height: wide ? 58 : 56, fontSize: wide ? 18 : 17, glow: ctaGlow))
            .keyboardShortcut(.defaultAction)
            .homeRise(revealed, 0.48)
        }
    }

    // MARK: Vos ordinateurs

    private func shelf(width: CGFloat, wide: Bool) -> some View {
        let gap: CGFloat = wide ? 16 : 12
        let m = wide ? HomeCardMetrics.pad(width: floor((width - 5 * gap) / 6)) : .phone
        return VStack(alignment: .leading, spacing: wide ? 14 : 10) {
            HStack(alignment: .firstTextBaseline) {
                Text("Vos ordinateurs".uppercased())
                    .font(Theme.mono(wide ? 11 : 10.5, .medium)).tracking(0.85).foregroundStyle(Theme.muted)
                Spacer(minLength: 12)
                if wide {
                    Text("Toucher une carte la met en scène · appui long : réglages, retirer")
                        .font(Theme.sans(13)).foregroundStyle(Theme.faint).lineLimit(1).minimumScaleFactor(0.8)
                } else if let latency = host?.lastLatencyMs {
                    // La règle des trames n'est pas ici : seulement la dernière latence, en pied.
                    Text("Dernière session \(HomeView.ms(latency)) ms")
                        .font(Theme.mono(10.5)).foregroundStyle(Theme.faint)
                }
            }
            .padding(.horizontal, wide ? 0 : 24)
            ScrollViewReader { proxy in
                ScrollView(.horizontal) {
                    HStack(spacing: gap) {
                        ForEach(store.ordered) { h in
                            card(h, m: m, wide: wide).id(h.id)
                        }
                        ForEach(app.newOnNetwork) { found in
                            Button { app.sheet = .addHost(resolved[found.id]) } label: {
                                HomeFoundCard(name: found.name, detail: resolved[found.id]?.address, m: m, wide: wide)
                            }
                            .buttonStyle(HomePressStyle())
                            .hoverEffect(.lift)
                            .transition(.opacity.combined(with: .scale(scale: 0.96)))
                            .accessibilityLabel(Text("\(found.name), sur le réseau. Ajouter"))
                        }
                        Button { app.sheet = .addHost(nil) } label: { HomeAddCard(m: m, wide: wide) }
                            .buttonStyle(HomePressStyle())
                            .hoverEffect(.lift)
                    }
                    .padding(.horizontal, wide ? 0 : 24)
                    .padding(.bottom, 8)
                    .animation(Theme.easeOut, value: app.newOnNetwork.map(\.id))
                }
                .scrollIndicators(.hidden)
                .scrollClipDisabled()
                .onChange(of: app.selected) { _, id in
                    guard let id else { return }
                    withAnimation(Theme.easeOut) { proxy.scrollTo(id, anchor: .center) }
                }
            }
        }
        .homeRise(revealed, 0.56)
    }

    private func card(_ h: Host, m: HomeCardMetrics, wide: Bool) -> some View {
        let on = online(h)
        return Button {
            withAnimation(Theme.easeOut) { app.selected = h.id }
        } label: {
            HomeHostCard(host: h, online: on, status: shortStatus(h, wide: wide), selected: h.id == host?.id, m: m)
        }
        .buttonStyle(HomePressStyle())
        .hoverEffect(.lift)
        .contextMenu {
            if on {
                Button { app.connect(h.id) } label: { Label { Text("Se connecter") } icon: { Image(systemName: "play.fill") } }
            }
            Button { app.sheet = .settings(h.id) } label: { Label { Text("Réglages") } icon: { Image(systemName: "slider.horizontal.3") } }
            Button(role: .destructive) { removing = h } label: { Label { Text("Retirer") } icon: { Image(systemName: "trash") } }
        }
        .accessibilityLabel(Text("\(h.name), \(shortStatus(h, wide: true))"))
        .accessibilityAddTraits(h.id == host?.id ? [.isSelected] : [])
        .accessibilityHint(Text("Met l'ordinateur en scène ; appui long : réglages, retirer"))
    }

    // MARK: Clavier (iPad)

    /// ↩ se connecte (bouton principal), ⌘, ouvre les réglages, ⌘N l'ajout,
    /// les flèches passent d'une carte à l'autre.
    private var shortcuts: some View {
        ZStack {
            Button("Réglages") { openSettings() }.keyboardShortcut(",", modifiers: .command)
            Button("Ajouter un ordinateur") { app.sheet = .addHost(nil) }.keyboardShortcut("n", modifiers: .command)
            Button("Ordinateur précédent") { move(-1) }.keyboardShortcut(.leftArrow, modifiers: [])
            Button("Ordinateur suivant") { move(1) }.keyboardShortcut(.rightArrow, modifiers: [])
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }

    private func move(_ step: Int) {
        let list = store.ordered
        guard !list.isEmpty, app.sheet == nil else { return }
        let i = list.firstIndex { $0.id == host?.id } ?? 0
        let next = min(max(i + step, 0), list.count - 1)
        withAnimation(Theme.easeOut) { app.selected = list[next].id }
    }

    // MARK: Actions

    private func retry(_ host: Host) {
        retrying = host.id
        Task {
            await store.refresh(host)
            retrying = nil
            if !(store.status[host.id]?.online ?? false) {
                app.show(String(localized: "\(host.name) ne répond pas encore"))
            }
        }
    }

    private func remove(_ h: Host) {
        let wasSelected = h.id == host?.id
        store.remove(h)
        removing = nil
        if wasSelected { app.selected = store.ordered.first?.id }
    }

    private var removeTitle: Text {
        let name = removing?.name ?? ""
        return isPad ? Text("Retirer \(name) de cet iPad ?") : Text("Retirer \(name) de cet iPhone ?")
    }

    /// L'allumage de la charte sur le bouton principal, une fois à l'arrivée
    /// de l'accueil (trois éclairs, puis il s'éteint) : le seul néon de l'écran.
    private func igniteCTA(after delay: Double) {
        guard !reduceMotion else { return }
        let steps: [(Double, NeonLevel)] = [(0.08, .flash), (0.14, .off), (0.22, .lit), (0.31, .off), (0.42, .flash), (0.52, .lit), (1.0, .hover), (1.4, .off)]
        for (t, level) in steps {
            DispatchQueue.main.asyncAfter(deadline: .now() + delay + t) { ctaGlow = level }
        }
    }

    /// L'adresse des ordinateurs annoncés : la carte l'affiche, l'ajout s'ouvre pré-rempli.
    private func resolveFound() async {
        for found in app.newOnNetwork where resolved[found.id] == nil {
            if let c = await Discovery.resolve(found) { resolved[found.id] = c }
        }
    }

    // MARK: Textes

    private func caption(_ h: Host) -> String {
        if let p = imagery.pictures[h.id] {
            let kind = p.source == .lastSession ? String(localized: "Dernière image") : String(localized: "Aperçu")
            return "\(kind) · \(HomeView.when(p.date))"
        }
        if let last = h.lastSession { return String(localized: "Dernière session · \(HomeView.when(last))") }
        return isPad ? String(localized: "Jamais ouvert sur cet iPad") : String(localized: "Jamais ouvert sur cet iPhone")
    }

    private func status(_ h: Host) -> String {
        guard online(h) else {
            if let last = h.lastSession { return String(localized: "Hors ligne · vu \(HomeView.ago(last))") }
            return String(localized: "Hors ligne")
        }
        return app.isLocal(h) ? String(localized: "En ligne · réseau local") : String(localized: "En ligne · par VPN")
    }

    private func shortStatus(_ h: Host, wide: Bool) -> String {
        guard online(h) else {
            if let last = h.lastSession { return String(localized: "Vu \(HomeView.ago(last, short: !wide))") }
            return String(localized: "Hors ligne")
        }
        return app.isLocal(h) ? String(localized: "Réseau local") : String(localized: "VPN")
    }

    private func specs(_ h: Host) -> String {
        let info = store.status[h.id]?.info
        var parts: [String] = []
        if let os = info?.os { parts.append(os.replacingOccurrences(of: " Pro", with: "").components(separatedBy: " ").prefix(2).joined(separator: " ")) }
        if let gpu = info?.gpu { parts.append(HomeView.gpu(gpu)) }
        parts.append(h.address)
        return parts.joined(separator: " · ")
    }

    private func screenLine(_ h: Host, online: Bool) -> String {
        guard online else { return String(localized: "Allumez-le ou sortez-le de veille : il réapparaîtra tout seul.") }
        let m = DeviceDisplay.mode(for: h.settings)
        if store.status[h.id]?.info?.virtualDisplay ?? true {
            let n = DeviceDisplay.native()
            guard m.width == n.width, m.height == n.height else {
                return String(localized: "Un écran de \(m.width) × \(m.height) à \(m.refresh) Hz l'attend.")
            }
            return isPad
                ? String(localized: "Un écran de \(m.width) × \(m.height) à \(m.refresh) Hz l'attend, taillé pour cet iPad.")
                : String(localized: "Un écran de \(m.width) × \(m.height) à \(m.refresh) Hz l'attend, taillé pour cet iPhone.")
        }
        return String(localized: "Son écran, jusqu'à \(m.width) × \(m.height) à \(m.refresh) Hz.")
    }

    private func hint(_ p: Profile?, wide: Bool) -> String {
        let hz = DeviceDisplay.native().refresh
        switch p {
        case .jeu?:
            return wide
                ? String(localized: "Jeu : latence minimale, \(hz) Hz, manette prête, souris en mode jeu quand un jeu est en plein écran.")
                : String(localized: "Jeu : latence minimale, \(hz) Hz, manette Bluetooth prête.")
        case .bureau?:
            return wide
                ? String(localized: "Bureau : texte net (4:4:4), aucune image perdue. Le trackpad est la souris, les doigts le pavé.")
                : String(localized: "Bureau : texte net (4:4:4), aucune image perdue.")
        case .max?:
            return wide
                ? String(localized: "Max : sans compromis, 4:4:4 et gros débit. En Wi-Fi 6 ou 7, ou avec l'adaptateur Ethernet.")
                : String(localized: "Max : sans compromis, 4:4:4 et gros débit. En Wi-Fi 6 ou 7 seulement.")
        case nil:
            return String(localized: "Personnalisé : vos réglages pour cet ordinateur.")
        }
    }
}

/// Petit cercle qui tourne pendant qu'on cherche l'ordinateur (« Réessayer »).
struct HomeSpinner: View {
    var size: CGFloat = 20
    @State private var turn = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Icon("arrows-clockwise", size: size, bold: true)
            .rotationEffect(.degrees(turn ? 360 : 0))
            .onAppear {
                guard !reduceMotion else { return }
                withAnimation(.linear(duration: 1).repeatForever(autoreverses: false)) { turn = true }
            }
    }
}
