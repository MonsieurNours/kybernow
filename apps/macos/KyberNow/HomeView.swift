// Accueil (maquettes « App · Accueil » et « App · Premier contact » ;
// docs/link-ui.md) : l'image de l'ordinateur en plein cadre, son état, son
// nom en grand, un seul geste pour entrer ; en bas, les autres ordinateurs
// (nom et état), ceux trouvés sur le réseau, l'ajout.

import KyberKit
import SwiftUI

struct HomeView: View {
    @EnvironmentObject var app: AppModel

    var body: some View {
        HomeContent(imagery: app.store.imagery, discovery: app.discovery)
            .sheet(isPresented: $app.adding) { AddHostSheet(initialLink: app.pendingLink, initialTab: app.addTab) }
            .preferredColorScheme(.dark)
            .frame(minWidth: 1100, minHeight: 760)
    }
}

private struct HomeContent: View {
    @EnvironmentObject var app: AppModel
    @ObservedObject var imagery: HostImagery
    @ObservedObject var discovery: Discovery
    @ObservedObject var updates = UpdateChannels.shared
    @Environment(\.openSettings) private var openSettings
    /// Arrivée de l'écran : cascade et allumage du symbole, une fois.
    @State private var arrived = false
    @State private var ignite = 0

    private var hosts: [Host] { app.store.ordered }
    private var featured: Host? { hosts.first { $0.id == app.selectedID } ?? hosts.first }
    private var others: [Host] { hosts.filter { $0.id != featured?.id } }

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .topLeading) {
                Theme.background
                backdrop
                legibility
                if let host = featured {
                    Hero(host: host, arrived: arrived, size: geo.size)
                        .id(host.id)
                        .transition(.opacity)
                        .padding(.leading, 72)
                        .padding(.top, max(96, geo.size.height * 0.15))
                } else {
                    Welcome(arrived: arrived)
                        .padding(.leading, 72)
                        .padding(.top, max(150, geo.size.height * 0.24))
                }
                Shelf(featured: featured, others: others, discovery: discovery, imagery: imagery, arrived: arrived)
                    .padding(.horizontal, 72)
                    .padding(.bottom, 34)
                    .frame(maxHeight: .infinity, alignment: .bottom)
                header
                if let toast = app.toast {
                    Toast(text: toast, warning: app.toastWarning)
                        .padding(24).padding(.top, 40)
                        .frame(maxWidth: .infinity, alignment: .topTrailing)
                        .transition(.move(edge: .top).combined(with: .opacity))
                }
            }
        }
        .ignoresSafeArea()
        .animation(.easeInOut(duration: 0.45), value: featured?.id)
        .onAppear {
            discovery.start()
            arrived = true
            ignite += 1
            #if DEBUG
            app.demoAppeared()
            if AppModel.demo, UserDefaults.standard.string(forKey: "screen") == "app" {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { openSettings() }
            }
            #endif
        }
    }

    @ViewBuilder private var backdrop: some View {
        if let host = featured {
            HostBackdrop(host: host, picture: imagery.pictures[host.id], online: app.store.status[host.id]?.online)
                .id(host.id)
                .transition(.opacity)
        } else {
            Signature(seed: "KyberNow")
        }
    }

    /// Voiles qui gardent le texte lisible, quel que soit le fond.
    private var legibility: some View {
        ZStack {
            LinearGradient(stops: [.init(color: Theme.background.opacity(0.88), location: 0), .init(color: Theme.background.opacity(0.55), location: 0.38), .init(color: .clear, location: 0.66)], startPoint: .leading, endPoint: .trailing)
            LinearGradient(stops: [.init(color: Theme.background, location: 0), .init(color: Theme.background.opacity(0.82), location: 0.24), .init(color: .clear, location: 0.52)], startPoint: .bottom, endPoint: .top)
            LinearGradient(colors: [Theme.background.opacity(0.55), .clear], startPoint: .top, endPoint: .init(x: 0.5, y: 0.14))
        }
        .allowsHitTesting(false)
    }

    /// Barre de titre transparente : symbole et nom, la mise à jour de l'app
    /// quand il y en a une, les réglages de l'app.
    private var header: some View {
        HStack(spacing: 10) {
            ZStack {
                Mark(size: 18)
                MarkIgnition(size: 18, trigger: ignite, delay: 0.3)
            }
            Wordmark(size: 14)
            Spacer()
            if let version = updates.available {
                Button { app.checkForUpdates() } label: {
                    HStack(spacing: 8) { Icon("arrows-clockwise", size: 15); Text("Version \(version) disponible") }
                }
                .buttonStyle(SignalButton(height: 32))
            }
            SettingsLink {
                HStack(spacing: 8) { Icon("gear-six", size: 16); Text("Réglages de l'app") }
            }
            .buttonStyle(GhostButton(height: 32))
        }
        .padding(.leading, 92)
        .padding(.trailing, 18)
        .frame(height: 56)
    }
}

// MARK: - L'ordinateur affiché

/// Pour l'ordinateur choisi, dans cet ordre et rien d'autre (docs/link-ui.md) :
/// état, nom, système, l'écran que la session demandera, « Se connecter »,
/// le profil et sa phrase, les réglages de l'ordinateur.
private struct Hero: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let arrived: Bool
    let size: CGSize

    private var width: CGFloat { size.width }
    /// Le nom en très grand, mais jamais au point de pousser le reste sur les autres ordinateurs.
    private var nameSize: CGFloat { min(104, size.width * 0.072, size.height * 0.095) }
    /// Assez de hauteur pour aérer les boutons.
    private var roomy: Bool { size.height > 920 }

    private var status: HostStatus? { app.store.status[host.id] }
    private var online: Bool? { status?.online }

    private var profile: Binding<Profile?> {
        Binding(get: { host.settings.profile }, set: { p in
            guard let p else { return }
            var h = host
            h.settings.apply(p)
            app.store.update(h)
        })
    }

    private var screenLine: String { HomeScreen.line(for: host, info: status?.info) }

    private func rise(_ i: Int) -> some ViewModifier { Rise(arrived: arrived, delay: 0.15 + Double(i) * 0.1) }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            StatusPill(online: online, text: LinkText.status(online))
                .modifier(rise(0))
            VStack(alignment: .leading, spacing: 10) {
                Text(host.name)
                    .font(Theme.display(nameSize, .bold))
                    .tracking(-nameSize * 0.045)
                    .lineLimit(2)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                    .frame(maxWidth: width * 0.56, alignment: .leading)
                if let os = status?.info?.os {
                    Text(verbatim: os).font(Theme.sans(16)).foregroundStyle(Theme.muted).lineLimit(1)
                }
                if let problem = status?.problem {
                    // Le réglage qui le règle : le mot de passe (Station), l'empreinte (Statistiques).
                    Button { app.openSettings(host, section: problem == .passwordRefused ? .station : .statistics) } label: {
                        HStack(spacing: 8) {
                            Icon("warning", size: 15, bold: true).foregroundStyle(Theme.warn)
                            Text(LinkText.problem(problem)).font(Theme.sans(14)).foregroundStyle(Theme.warnText)
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .modifier(rise(1))
            HStack(spacing: 10) {
                Icon("monitor", size: 18).foregroundStyle(Theme.accent)
                Text(screenLine).font(Theme.sans(15)).foregroundStyle(Theme.text2)
            }
            .modifier(rise(2))
            actions
                .padding(.top, roomy ? 22 : 10)
                .modifier(rise(3))
            Text(Profile.hint(host.settings.profile))
                .font(Theme.sans(13.5))
                .foregroundStyle(Theme.muted)
                .modifier(rise(4))
        }
        .frame(maxWidth: 820, alignment: .leading)
    }

    @ViewBuilder private var actions: some View {
        HStack(spacing: 14) {
            if online == false, app.store.waking.contains(host.id) {
                // Réveil en cours : l'ordinateur est sondé toutes les 2 s.
                Button {} label: {
                    HStack(spacing: 10) { ProgressView().controlSize(.small); Text(LinkText.waking) }
                }
                .buttonStyle(GhostButton(height: 56))
                .disabled(true)
            } else if online == false, app.store.canWake(host) {
                Button { app.store.wake(host) } label: {
                    HStack(spacing: 10) { Icon("power", size: 18, bold: true); Text(LinkText.wake) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
            } else if online == false {
                Button {
                    Task { await app.store.refresh(host) }
                } label: {
                    HStack(spacing: 10) { Icon("arrows-clockwise", size: 18); Text("Réessayer") }
                }
                .buttonStyle(GhostButton(height: 56))
            } else {
                Button { app.connect(host) } label: {
                    HStack(spacing: 12) {
                        Icon("arrow-elbow-down-left", size: 14, bold: true)
                            .padding(.horizontal, 7).padding(.vertical, 4)
                            .background(Color.black.opacity(0.18), in: RoundedRectangle(cornerRadius: 5))
                        Text("Se connecter")
                        Icon("arrow-right", size: 18, bold: true)
                    }
                }
                .buttonStyle(PrimaryButton(height: 56, lit: app.session != nil))
                .keyboardShortcut(.defaultAction)
                .disabled(app.session != nil)
            }
            Segmented(options: Profile.allCases.map { ($0, $0.label) }, selection: profile,
                      icons: [.jeu: "game-controller", .bureau: "article", .max: "lightning"], height: 40)
                .fixedSize()
            Button { app.openSettings(host) } label: {
                HStack(spacing: 8) { Icon("gear-six", size: 17); Text("Réglages") }
            }
            .buttonStyle(GhostButton(height: 48))
            .help(LinkText.settingsTitle(host.name))
            .accessibilityLabel(LinkText.settingsTitle(host.name))
        }
    }
}

/// L'écran que la session demandera, dit comme sur toutes les apps : même
/// calcul que la session (`SessionController.start`).
enum HomeScreen {
    static func line(for host: Host, info: HostInfo?) -> String {
        let s = host.settings
        let screen = NSScreen.main ?? NSScreen.screens[0]
        let virtual = s.display != .physical && (info?.virtualDisplay ?? true)
        // Plusieurs écrans : le nombre que prendra la session, calculé par le cœur.
        if let count = HomeScreen.screenCount(for: host, info: info) {
            return LinkText.screens(count)
        }
        switch s.display {
        case .custom:
            return LinkText.screen(width: s.customWidth, height: s.customHeight, refresh: s.customRefresh, virtual: virtual, sizedForThisScreen: false)
        case .window:
            // Session fenêtrée : l'écran prendra la taille de la fenêtre, en pixels.
            let window = NSApp.keyWindow ?? NSApp.mainWindow
            let content = window?.contentLayoutRect.size ?? screen.frame.size
            let scale = CGFloat(DisplayInfo.nativePixels(screen).width) / screen.frame.width
            let w = Int(content.width * scale) / 2 * 2, h = Int(content.height * scale) / 2 * 2
            guard virtual else { return LinkText.screen(width: w, height: h, refresh: 0, virtual: false, sizedForThisScreen: false) }
            return String(localized: "Écran de \(LinkFormat.dims(w, h)) à \(max(30, screen.maximumFramesPerSecond)) Hz, à la taille de cette fenêtre")
        case .match, .physical:
            let m = DisplayInfo.mode(for: screen, codec: s.codec, limit4K: s.limit4K)
            return LinkText.screen(width: m.width, height: m.height, refresh: m.refresh, virtual: virtual, sizedForThisScreen: !s.limit4K || m.width == DisplayInfo.mode(for: screen, codec: s.codec).width)
        }
    }
}

extension HomeScreen {
    /// Écrans que prendra la session (au moins deux), nil pour un : le plan
    /// du cœur, le même que celui de la session (docs/multi-ecrans.md).
    static func screenCount(for host: Host, info: HostInfo?) -> Int? {
        let s = host.settings
        let virtual = s.display != .physical && (info?.virtualDisplay ?? true)
        let current = (NSApp.keyWindow ?? NSApp.mainWindow)?.screen ?? NSScreen.main
        let request = DisplayInfo.deviceScreens(current: current, settings: s, maxScreens: DisplayInfo.maxScreens(info)).request
        return SessionEngine.sessionPlan(request, profile: s.profile?.rawValue, virtualDisplay: virtual, codec: s.codec)?.count
    }
}

/// Aucun ordinateur pour l'instant.
private struct Welcome: View {
    @EnvironmentObject var app: AppModel
    let arrived: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Ouvrez votre ordinateur sur ce Mac")
                .font(Theme.display(64, .bold)).tracking(-64 * 0.045)
                .foregroundStyle(Theme.text)
                .frame(maxWidth: 900, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
                .modifier(Rise(arrived: arrived, delay: 0.25))
            Text("Ajoutez un ordinateur équipé de KyberNow Station. Sur ce réseau, il apparaît tout seul en bas de l'écran.")
                .font(Theme.sans(16)).foregroundStyle(Theme.text2)
                .frame(maxWidth: 560, alignment: .leading)
                .modifier(Rise(arrived: arrived, delay: 0.35))
            Button { app.add() } label: {
                HStack(spacing: 12) { Icon("plus", size: 18, bold: true); Text("Ajouter un ordinateur") }
            }
            .buttonStyle(PrimaryButton(height: 56))
            .padding(.top, 18)
            .modifier(Rise(arrived: arrived, delay: 0.45))
        }
    }
}

// MARK: - Les autres ordinateurs

private struct Shelf: View {
    @EnvironmentObject var app: AppModel
    let featured: Host?
    let others: [Host]
    @ObservedObject var discovery: Discovery
    @ObservedObject var imagery: HostImagery
    let arrived: Bool
    /// Annonces dont l'adresse a été obtenue : une console répond vraiment
    /// derrière. Une annonce périmée (console arrêtée sans retirer son
    /// annonce, gardée en cache jusqu'à 75 min) ne se résout pas : on la tait.
    @State private var resolved: [String: Candidate] = [:]

    /// Annoncés sur le réseau, joignables, pas encore ajoutés.
    private var newcomers: [(found: Discovery.Found, candidate: Candidate)] {
        discovery.found.compactMap { f in
            guard let c = resolved[f.id] else { return nil }
            let known = app.store.hosts.contains { h in
                (f.fingerprint != nil && h.consoleFingerprint == f.fingerprint) || (h.address == c.address && h.consolePort == c.port)
            }
            return known ? nil : (f, c)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Theme.eyebrow(featured == nil ? String(localized: "Sur ce réseau") : String(localized: "Vos autres ordinateurs"), color: Theme.muted)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 16) {
                    ForEach(Array(others.enumerated()), id: \.element.id) { i, host in
                        HostCard(host: host, picture: imagery.pictures[host.id]) {
                            app.selectedID = host.id
                        }
                        .modifier(Rise(arrived: arrived, delay: 0.8 + Double(i) * 0.08))
                    }
                    ForEach(newcomers, id: \.found.id) { item in
                        FoundCard(found: item.found, candidate: item.candidate)
                            .modifier(Rise(arrived: arrived, delay: 1.0))
                            .transition(.scale(scale: 0.96).combined(with: .opacity))
                    }
                    AddCard { app.add() }
                        .modifier(Rise(arrived: arrived, delay: 1.1))
                }
                .padding(.vertical, 18)
                .padding(.horizontal, 2)
                .animation(Theme.easeOut, value: newcomers.map(\.found.id))
            }
            .task(id: discovery.found.map(\.id)) {
                for found in discovery.found where resolved[found.id] == nil {
                    if let c = await Discovery.resolve(found) { resolved[found.id] = c }
                }
            }
            .padding(.vertical, -18)
            .scrollClipDisabled()
        }
    }
}

private let cardSize = CGSize(width: 244, height: 164)

/// Un autre ordinateur : son nom et son état, rien d'autre.
private struct HostCard: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let picture: HostImagery.Picture?
    let select: () -> Void
    @State private var hovering = false

    private var status: HostStatus? { app.store.status[host.id] }

    var body: some View {
        Button(action: select) {
            VStack(alignment: .leading, spacing: 0) {
                HostBackdrop(host: host, picture: picture, online: status?.online, drift: false)
                    .frame(height: 86)
                    .clipped()
                VStack(alignment: .leading, spacing: 4) {
                    Text(host.name).font(Theme.sans(15, .semibold)).foregroundStyle(status?.online == false ? Theme.muted : Theme.text).lineLimit(1)
                    HStack(spacing: 7) {
                        Circle()
                            .fill(status?.online == true ? Theme.ok : .clear)
                            .overlay(Circle().strokeBorder(status?.online == true ? .clear : Theme.faint))
                            .frame(width: 6, height: 6)
                        Text(LinkText.status(status?.online)).font(Theme.sans(12)).foregroundStyle(Theme.muted).lineLimit(1)
                    }
                }
                .padding(.horizontal, 14).padding(.vertical, 11)
            }
            .frame(width: cardSize.width, height: cardSize.height, alignment: .top)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .glass(14)
            .offset(y: hovering ? -3 : 0)
            .shadow(color: .black.opacity(hovering ? 0.5 : 0), radius: 18, y: 12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { h in withAnimation(Theme.easeOut) { hovering = h } }
        .contextMenu {
            Button("Se connecter") { app.connect(host) }.disabled(status?.online != true || app.session != nil)
            Button(LinkText.settingsTitle(host.name)) { app.openSettings(host) }
        }
    }
}

/// Ordinateur annoncé sur le réseau : il s'allume en apparaissant, puis reste en veille.
private struct FoundCard: View {
    @EnvironmentObject var app: AppModel
    let found: Discovery.Found
    let candidate: Candidate
    @State private var ignite = 0
    @State private var breathe = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 7) {
                    Icon("wifi-high", size: 14)
                    Theme.eyebrow(String(localized: "Sur le réseau"), color: Theme.accentLight)
                }
                .foregroundStyle(Theme.accentLight)
                Text(found.name).font(Theme.display(22, .semibold)).tracking(-0.4).foregroundStyle(Theme.text).lineLimit(1)
            }
            Spacer(minLength: 0)
            Button("Ajouter") { app.add(link: candidate) }
            .buttonStyle(SignalButton(height: 32))
        }
        .padding(16)
        .frame(width: cardSize.width, height: cardSize.height, alignment: .topLeading)
        .background(Theme.accentFill.opacity(0.12), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.accent.opacity(breathe ? 0.5 : 0.75)))
        .shadow(color: Theme.accentFill.opacity(breathe ? 0.2 : 0.35), radius: breathe ? 10 : 18)
        .neonIgnition(trigger: ignite, delay: 0.6)
        .task {
            ignite += 1
            if !reduceMotion {
                try? await Task.sleep(for: .seconds(1.8))
                withAnimation(.easeInOut(duration: 1.6).repeatForever(autoreverses: true)) { breathe = true }
            }
        }
    }
}

private struct AddCard: View {
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 12) {
                Icon("plus", size: 22, bold: true).foregroundStyle(Theme.accent)
                Spacer()
                Text("Ajouter un ordinateur").font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
            }
            .padding(16)
            .frame(width: cardSize.width, height: cardSize.height, alignment: .topLeading)
            .background {
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(Theme.background.opacity(0.5))
                    .overlay(Hatching().clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous)))
            }
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Color.white.opacity(hovering ? 0.2 : 0.1)))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
    }
}

/// Hachures à 45° très discrètes (la carte « ajouter »).
private struct Hatching: View {
    var body: some View {
        Canvas { ctx, size in
            var p = Path()
            var x: CGFloat = -size.height
            while x < size.width {
                p.move(to: CGPoint(x: x, y: size.height))
                p.addLine(to: CGPoint(x: x + size.height, y: 0))
                x += 10
            }
            ctx.stroke(p, with: .color(Color.white.opacity(0.03)), lineWidth: 1)
        }
    }
}

// MARK: - Outils

/// Arrivée en cascade : opacité et petite montée, une fois.
struct Rise: ViewModifier {
    let arrived: Bool
    let delay: Double
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content
            .opacity(arrived || reduceMotion ? 1 : 0)
            .offset(y: arrived || reduceMotion ? 0 : 18)
            .animation(Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.8).delay(delay), value: arrived)
    }
}

/// Dates, durées et mesures dites comme on les dit.
enum Moment {
    /// Français : « 14 h 05 », « le 3 octobre » ; anglais : heure et date
    /// dans le format de la région du Mac.
    private static var french: Bool { LanguageChoice.effective == "fr" }

    static func describe(_ date: Date, now: Date = Date()) -> String {
        let cal = Calendar.current
        let time = french
            ? date.formatted(.dateTime.hour(.twoDigits(amPM: .omitted)).minute(.twoDigits).locale(Locale(identifier: "fr_FR")))
                .replacingOccurrences(of: ":", with: " h ")
            : date.formatted(date: .omitted, time: .shortened)
        if cal.isDateInToday(date) {
            return now.timeIntervalSince(date) < 120 ? String(localized: "à l'instant") : String(localized: "aujourd'hui, \(time)")
        }
        if cal.isDateInYesterday(date) { return String(localized: "hier, \(time)") }
        let days = cal.dateComponents([.day], from: cal.startOfDay(for: date), to: cal.startOfDay(for: now)).day ?? 0
        if days < 7 { return String(localized: "il y a \(days) jours") }
        let day = french
            ? date.formatted(.dateTime.day().month(.wide).locale(Locale(identifier: "fr_FR")))
            : date.formatted(.dateTime.day().month(.wide))
        return String(localized: "le \(day)")
    }

    static func duration(_ seconds: TimeInterval) -> String {
        let minutes = Int(seconds / 60)
        if minutes < 60 { return "\(max(1, minutes)) min" }
        let rest = String(format: "%02d", minutes % 60)
        return "\(minutes / 60) h \(rest)"
    }

    /// Une décimale sous 100 ms, virgule en français, point en anglais.
    static func ms(_ value: Double, unit: Bool = true) -> String {
        var text = value >= 100 ? String(Int(value.rounded())) : String(format: "%.1f", value)
        if french { text = text.replacingOccurrences(of: ".", with: ",") }
        return unit ? "\(text) ms" : text
    }
}

struct Toast: View {
    let text: String
    var warning = false

    var body: some View {
        HStack(spacing: 12) {
            Icon(warning ? "warning" : "check", size: 14, bold: true)
                .foregroundStyle(warning ? Theme.warn : Theme.ok)
            Text(text).font(Theme.sans(13)).foregroundStyle(Theme.text)
        }
        .padding(.horizontal, 16).padding(.vertical, 13)
        .glass(12, tint: 0.6)
        .shadow(color: .black.opacity(0.45), radius: 20, y: 10)
    }
}
