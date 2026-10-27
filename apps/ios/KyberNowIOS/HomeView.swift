// Accueil (maquettes iOS, planche 1 : « Accueil iPhone » et « Accueil
// iPad ») : l'accueil du Mac et la scène de l'Apple TV, au doigt. La dernière
// image de l'ordinateur choisi occupe tout l'écran ; dessus, dans l'ordre de
// docs/link-ui.md et rien d'autre : son état, son nom, son système, l'écran
// que la session demandera, « Se connecter », le profil et sa phrase, ses
// réglages. En bas, la rangée « Vos ordinateurs » (nom et état) qui défile.
// Toucher une carte met son ordinateur en scène, un appui long propose ses
// réglages ou de le retirer ; + et le QR code ouvrent l'ajout. Les chiffres
// (latence, carte graphique, adresse) sont dans les réglages de l'ordinateur,
// rubrique Statistiques.
//
// iPad en pleine largeur : la composition de l'Apple TV ; en largeur réduite
// (Split View), celle de l'iPhone.
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

}

// MARK: - L'écran

struct HomeScreen: View {
    @ObservedObject var app: AppModel
    @ObservedObject var store: HostStore
    @ObservedObject var discovery: Discovery
    @ObservedObject var cloud: CloudSync
    @ObservedObject var imagery: HostImagery
    @ObservedObject var pads: GamepadBridge
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
    /// En ligne, hors ligne, ou nil tant qu'il n'a pas répondu une fois (« Recherche… »).
    private func online(_ h: Host) -> Bool? { store.status[h.id]?.online }

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .topLeading) {
                HomeBackdrop(host: host, online: host.map { online($0) != false } ?? true, wide: wide)
                    .ignoresSafeArea()
                    .opacity(sceneShown ? 1 : 0)
                if wide { wideLayout(geo.size) } else { compactLayout(geo.size) }
            }
            .frame(width: geo.size.width, height: geo.size.height)
            .overlay(alignment: .topLeading) { flyingBrand(geo.size) }
        }
        .coordinateSpace(.named(Self.space))
        .background(shortcuts)
        .confirmationDialog(StationText.removeQuestion(removing?.name ?? ""), isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }), titleVisibility: .visible) {
            Button(StationText.removeConfirm, role: .destructive) { if let h = removing { remove(h) } }
            Button(StationText.cancel, role: .cancel) {}
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

    /// Le symbole et le nom ; à droite, l'ajout (QR code, +) et les réglages de l'app.
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
                // KyberNow Share : rejoindre le partage de quelqu'un (sur iPhone : la carte de la rangée).
                if wide {
                    Button { app.sheet = .join(nil) } label: {
                        HStack(spacing: 10) {
                            Icon("share-network", size: 18)
                            Text(ShareText.join)
                        }
                    }
                    .buttonStyle(HomePillStyle())
                }
                if wide {
                    Button { app.sheet = .appSettings } label: {
                        HStack(spacing: 10) {
                            Icon("sliders-horizontal", size: 18)
                            Text("Réglages de l'app")
                        }
                    }
                    .buttonStyle(HomePillStyle())
                } else {
                    Button { app.sheet = .appSettings } label: { Icon("sliders-horizontal", size: 20) }
                        .buttonStyle(HomeCircleStyle())
                        .accessibilityLabel(Text("Réglages de l'app"))
                }
            }
            .homeRise(revealed, 0.1)
        }
        .padding(.leading, wide ? 0 : 4)
    }

    /// Les réglages de l'ordinateur en scène : l'engrenage, à côté du profil.
    private func settingsButton(_ host: Host, wide: Bool) -> some View {
        Button { app.sheet = .settings(host.id) } label: {
            if wide {
                HStack(spacing: 9) {
                    Icon("gear-six", size: 20)
                    Text("Réglages")
                }
                .font(Theme.sans(15, .medium))
                .foregroundStyle(Theme.text)
                .padding(.leading, 14).padding(.trailing, 18)
                .frame(height: 52)
                .glass(12)
                .contentShape(Rectangle())
            } else {
                Icon("gear-six", size: 22)
                    .foregroundStyle(Theme.text)
                    .frame(width: 52, height: 52)
                    .glass(12)
                    .contentShape(Rectangle())
            }
        }
        .buttonStyle(HomePressStyle())
        .hoverEffect(.highlight)
        .accessibilityLabel(Text(LinkText.settingsTitle(host.name)))
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
        let margin: CGFloat = size.width >= 1000 ? 72 : 48
        return VStack(spacing: 0) {
            topBar
                .frame(height: 48)
                .padding(.top, 16)
                .padding(.horizontal, margin)
                .zIndex(1)
            ScrollView(.vertical) {
                VStack(alignment: .leading, spacing: 0) {
                    VStack(alignment: .leading, spacing: 18) {
                        hero(wide: true)
                        actions(wide: true).padding(.top, 18)
                    }
                    .frame(maxWidth: 820, alignment: .leading)
                    Spacer(minLength: 40)
                    shelf(width: size.width - 2 * margin, wide: true)
                        .padding(.bottom, 24)
                }
                // Portrait : l'ordinateur un peu plus bas, au tiers de l'écran.
                .padding(.top, size.height > 900 ? max(108, size.height * 0.16) : 48)
                .padding(.horizontal, margin)
                .frame(minHeight: size.height - 64, alignment: .top)
            }
            .scrollBounceBehavior(.basedOnSize)
            .scrollIndicators(.hidden)
        }
    }

    // MARK: L'ordinateur en scène

    @ViewBuilder private func hero(wide: Bool) -> some View {
        let gap: CGFloat = wide ? 16 : 11
        if let host {
            let on = online(host)
            let info = store.status[host.id]?.info
            let problem = store.status[host.id]?.problem
            VStack(alignment: .leading, spacing: gap) {
                HomeChip(height: wide ? 32 : 30, fontSize: wide ? 14 : 13) {
                    HomeStatusDot(online: on, size: wide ? 8 : 7)
                    Text(LinkText.status(on)).lineLimit(1)
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
                if let os = info?.os {
                    Text(verbatim: os)
                        .font(Theme.sans(wide ? 17 : 14.5))
                        .foregroundStyle(Theme.text2)
                        .lineLimit(1)
                        .homeRise(revealed, 0.24)
                }
                if let problem {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Icon("warning", size: wide ? 17 : 15).foregroundStyle(Theme.warn)
                            .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 3 }
                        Text(LinkText.problem(problem))
                            .font(Theme.sans(wide ? 15 : 13.5))
                            .foregroundStyle(Theme.warnText)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .homeRise(revealed, 0.28)
                }
                HStack(alignment: .firstTextBaseline, spacing: wide ? 10 : 9) {
                    Icon("monitor", size: wide ? 19 : 17).foregroundStyle(Theme.accent)
                        .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 3 }
                    Text(screenLine(host))
                        .font(Theme.sans(wide ? 16 : 13.5))
                        .foregroundStyle(Theme.text2)
                        .lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .homeRise(revealed, 0.32)
            }
            .animation(Theme.easeOut, value: on)
        } else {
            VStack(alignment: .leading, spacing: gap) {
                HStack(spacing: 9) {
                    Circle().fill(Theme.accent).frame(width: 6, height: 6)
                    Theme.eyebrow(String(localized: "Bienvenue"))
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

    // MARK: Se connecter, puis le profil

    @ViewBuilder private func actions(wide: Bool) -> some View {
        if let host {
            // « Recherche… » : Se connecter, comme en ligne. Hors ligne :
            // « Réveiller l'ordinateur » si l'app connaît une de ses cartes
            // réseau (le paquet part, puis l'ordinateur est sondé toutes les 2 s),
            // sinon « Réessayer ».
            let on = online(host) != false
            let canWake = !on && store.canWake(host)
            let waking = store.waking.contains(host.id)
            let busy = retrying == host.id || waking
            let cta = Button {
                if on { app.connect(host.id) } else if canWake { store.wake(host) } else { retry(host) }
            } label: {
                HStack(spacing: 12) {
                    Text(on ? String(localized: "Se connecter") : waking ? LinkText.waking : canWake ? LinkText.wake : String(localized: "Réessayer"))
                    if !wide { Spacer(minLength: 0) }
                    if busy {
                        HomeSpinner(size: 20)
                    } else {
                        Icon("arrow-right", size: 20, bold: true)
                    }
                }
                .padding(.leading, wide ? 22 : 20)
                .padding(.trailing, wide ? 26 : 18)
                .frame(maxWidth: wide ? nil : .infinity)
            }
            // Le réveil est une action franche, comme se connecter : même bouton allumé.
            .buttonStyle(HomeCTAStyle(online: on || canWake, height: wide ? 58 : 56, fontSize: wide ? 18 : 17,
                                      glow: hovering && on ? max(ctaGlow, .hover) : ctaGlow))
            .keyboardShortcut(.defaultAction)
            .onHover { hovering = $0 }
            .hoverEffect(.highlight)
            .disabled(busy)
            .accessibilityHint(on ? Text("Ouvre la session sur \(host.name)") : canWake ? Text(SettingsText.wakeHelp) : Text("Cherche l'ordinateur à nouveau"))

            let picker = HomeProfilePicker(selected: host.settings.profile, fill: !wide, height: 44, fontSize: wide ? 15 : 14) { p in
                var h = host
                h.settings.apply(p)
                store.update(h)
            }
            let hint = Text(Profile.hint(host.settings.profile))
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
                        settingsButton(host, wide: true)
                    }
                    hint
                }
                .homeRise(revealed, 0.48)
                .animation(Theme.easeOut, value: host.settings.profile)
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    cta
                    HStack(spacing: 8) {
                        picker
                        settingsButton(host, wide: false)
                    }
                    hint
                }
                .homeRise(revealed, 0.48)
                .animation(Theme.easeOut, value: host.settings.profile)
            }
        } else {
            VStack(alignment: .leading, spacing: 12) {
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
            // KyberNow Share : rejoindre le partage de quelqu'un, sans ordinateur à soi.
            Button { app.sheet = .join(nil) } label: {
                HStack(spacing: 10) {
                    Icon("share-network", size: 18)
                    Text(ShareText.join)
                }
                .font(Theme.sans(16, .medium))
                .foregroundStyle(Theme.text)
                .padding(.horizontal, 20)
                .frame(maxWidth: wide ? nil : .infinity, minHeight: 50)
                .glass(12)
                .contentShape(Rectangle())
            }
            .buttonStyle(HomePressStyle())
            .homeRise(revealed, 0.52)
            }
        }
    }

    // MARK: Vos ordinateurs

    private func shelf(width: CGFloat, wide: Bool) -> some View {
        let gap: CGFloat = wide ? 16 : 12
        let m = wide ? HomeCardMetrics.pad(width: floor((width - 5 * gap) / 6)) : .phone
        return VStack(alignment: .leading, spacing: wide ? 14 : 10) {
            Text(String(localized: "Vos ordinateurs").uppercased())
                .font(Theme.mono(wide ? 11 : 10.5, .medium)).tracking(0.85).foregroundStyle(Theme.muted)
                .padding(.horizontal, wide ? 0 : 24)
                .accessibilityAddTraits(.isHeader)
            ScrollViewReader { proxy in
                ScrollView(.horizontal) {
                    HStack(spacing: gap) {
                        ForEach(store.ordered) { h in
                            card(h, m: m, wide: wide).id(h.id)
                        }
                        ForEach(app.newOnNetwork) { found in
                            Button { app.sheet = .addHost(resolved[found.id]) } label: {
                                HomeFoundCard(name: found.name, m: m, wide: wide)
                            }
                            .buttonStyle(HomePressStyle())
                            .hoverEffect(.lift)
                            .transition(.opacity.combined(with: .scale(scale: 0.96)))
                            .accessibilityLabel(Text("\(found.name), sur le réseau. Ajouter"))
                        }
                        Button { app.sheet = .addHost(nil) } label: { HomeAddCard(m: m, wide: wide) }
                            .buttonStyle(HomePressStyle())
                            .hoverEffect(.lift)
                        Button { app.sheet = .join(nil) } label: { HomeJoinCard(m: m, wide: wide) }
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
            HomeHostCard(host: h, online: on, status: LinkText.status(on), selected: h.id == host?.id, m: m)
        }
        .buttonStyle(HomePressStyle())
        .hoverEffect(.lift)
        .contextMenu {
            if on != false {
                Button { app.connect(h.id) } label: { Label { Text("Se connecter") } icon: { Image(systemName: "play.fill") } }
            }
            Button { app.sheet = .settings(h.id) } label: { Label { Text("Réglages") } icon: { Image(systemName: "slider.horizontal.3") } }
            Button(role: .destructive) { removing = h } label: { Label { Text("Retirer") } icon: { Image(systemName: "trash") } }
        }
        .accessibilityLabel(Text(verbatim: "\(h.name), \(LinkText.status(on))"))
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

    /// L'écran que la session demandera (même calcul que la session) :
    /// virtuel, à la taille de cet écran ou sur mesure, ou ceux de l'ordinateur
    /// tels quels (réglage coupé, ou Station sans écran virtuel).
    private func screenLine(_ h: Host) -> String {
        let m = DeviceDisplay.mode(for: h.settings)
        let virtual = h.settings.display != .physical && (store.status[h.id]?.info?.virtualDisplay ?? true)
        return LinkText.screen(width: m.width, height: m.height, refresh: m.refresh, virtual: virtual,
                               sizedForThisScreen: h.settings.display != .custom)
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
