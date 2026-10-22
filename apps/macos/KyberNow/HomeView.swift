// Accueil (maquettes « App · Accueil » et « App · Premier contact ») :
// l'image de l'ordinateur en plein cadre, son nom en grand, un seul geste pour
// entrer ; en bas, les autres ordinateurs, ceux trouvés sur le réseau, l'ajout.

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
    @State private var selectedID: UUID?
    /// Arrivée de l'écran : cascade et allumage du symbole, une fois.
    @State private var arrived = false
    @State private var ignite = 0

    private var hosts: [Host] { app.store.ordered }
    private var featured: Host? { hosts.first { $0.id == selectedID } ?? hosts.first }
    private var others: [Host] { hosts.filter { $0.id != featured?.id } }

    var body: some View {
        GeometryReader { geo in
            ZStack(alignment: .topLeading) {
                Theme.background
                backdrop
                legibility
                if let host = featured {
                    Hero(host: host, picture: imagery.pictures[host.id], fetching: imagery.fetching.contains(host.id), arrived: arrived, size: geo.size)
                        .id(host.id)
                        .transition(.opacity)
                        .padding(.leading, 72)
                        .padding(.top, max(96, geo.size.height * 0.15))
                    if let ms = host.lastLatencyMs {
                        LastSessionPanel(median: ms, p95: host.lastLatencyP95Ms, duration: host.lastDuration)
                            .frame(width: 300)
                            .padding(.trailing, 72)
                            .frame(maxWidth: .infinity, alignment: .trailing)
                            .padding(.top, max(330, geo.size.height * 0.46))
                            .opacity(arrived ? 1 : 0)
                            .offset(x: arrived ? 0 : 24)
                            .animation(Theme.easeOut.delay(0.7), value: arrived)
                    }
                } else {
                    Welcome(arrived: arrived)
                        .padding(.leading, 72)
                        .padding(.top, max(150, geo.size.height * 0.24))
                }
                Shelf(featured: featured, others: others, selectedID: $selectedID, discovery: discovery, imagery: imagery, arrived: arrived)
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

    /// Barre de titre transparente : symbole et nom, puis les actions de l'app.
    private var header: some View {
        HStack(spacing: 10) {
            ZStack {
                Mark(size: 18)
                MarkIgnition(size: 18, trigger: ignite, delay: 0.3)
            }
            Wordmark(size: 14)
            Spacer()
            Button { app.add(tab: .link) } label: {
                HStack(spacing: 8) { Icon("qr-code", size: 16); Text("Scanner un QR code") }
            }
            .buttonStyle(GhostButton(height: 32))
            Button { app.add() } label: { Icon("plus", size: 16) }
                .buttonStyle(GhostButton(height: 32))
                .help("Ajouter un ordinateur")
                .accessibilityLabel("Ajouter un ordinateur")
            if let host = featured {
                Button { app.route = .settings(host.id) } label: { Icon("sliders-horizontal", size: 16) }
                    .buttonStyle(GhostButton(height: 32))
                    .help("Réglages de \(host.name)")
                    .accessibilityLabel("Réglages de \(host.name)")
            }
        }
        .padding(.leading, 92)
        .padding(.trailing, 18)
        .frame(height: 56)
    }
}

// MARK: - L'ordinateur affiché

private struct Hero: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let picture: HostImagery.Picture?
    let fetching: Bool
    let arrived: Bool
    let size: CGSize

    private var width: CGFloat { size.width }
    /// Le nom en très grand, mais jamais au point de pousser le reste sur les autres ordinateurs.
    private var nameSize: CGFloat { min(104, size.width * 0.072, size.height * 0.095) }
    /// Assez de hauteur pour montrer d'où vient l'image ?
    private var roomy: Bool { size.height > 920 }

    private var status: HostStatus? { app.store.status[host.id] }
    private var online: Bool? { status?.online }
    private var neverOpened: Bool { host.lastSession == nil }

    private var profile: Binding<Profile?> {
        Binding(get: { host.settings.profile }, set: { p in
            guard let p else { return }
            var h = host
            h.settings.apply(p)
            app.store.update(h)
        })
    }

    private var eyebrow: String {
        switch picture?.source {
        case .lastSession: return String(localized: "Dernière image · \(Moment.describe(picture!.date))")
        case .preview:
            let fresh = Date().timeIntervalSince(picture!.date) < 120
            let when = fresh ? String(localized: "à l'instant") : Moment.describe(picture!.date)
            return String(localized: "Aperçu · \(when), depuis la Station")
        case nil:
            if neverOpened { return String(localized: "Nouvel ordinateur · jamais ouvert depuis ce Mac") }
            return host.settings.keepLastImage ? String(localized: "Sa signature · pas encore d'image") : String(localized: "Sa signature · aucune image gardée")
        }
    }

    private var meta: String {
        [status?.info?.os, status?.info?.gpu, host.address].compactMap { $0 }.joined(separator: " · ")
    }

    private var screenLine: String {
        switch host.settings.display {
        case .physical: return String(localized: "Ses écrans tels quels, sans écran virtuel.")
        case .window: return String(localized: "Un écran à la taille de cette fenêtre l'attend.")
        case .custom: return String(localized: "Un écran de \(host.settings.customWidth) × \(host.settings.customHeight) à \(host.settings.customRefresh) Hz l'attend.")
        case .match:
            let m = DisplayInfo.mode(for: NSScreen.main ?? NSScreen.screens[0], codec: host.settings.codec, limit4K: host.settings.limit4K)
            return String(localized: "Un écran de \(m.width) × \(m.height) à \(m.refresh) Hz l'attend, taillé pour cet écran.")
        }
    }

    private var statusText: String {
        switch online {
        case nil: return String(localized: "Recherche…")
        case true?: return String(localized: "En ligne")
        case false?: return String(localized: "Hors ligne")
        }
    }

    private func rise(_ i: Int) -> some ViewModifier { Rise(arrived: arrived, delay: 0.15 + Double(i) * 0.1) }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(spacing: 10) {
                Circle().fill(picture == nil ? Theme.faint : Theme.accent).frame(width: 6, height: 6)
                Text(eyebrow.uppercased()).font(Theme.mono(11, .medium)).tracking(0.9).foregroundStyle(Theme.muted)
            }
            .modifier(rise(0))
            Text(host.name)
                .font(Theme.display(nameSize, .bold))
                .tracking(-nameSize * 0.045)
                .lineLimit(2)
                .minimumScaleFactor(0.5)
                .foregroundStyle(Theme.text)
                .frame(maxWidth: width * 0.56, alignment: .leading)
                .modifier(rise(1))
            HStack(spacing: 14) {
                StatusPill(online: online, text: statusText)
                Text(meta).font(Theme.mono(12)).foregroundStyle(Theme.muted).lineLimit(1)
            }
            .modifier(rise(2))
            HStack(spacing: 10) {
                Icon("monitor", size: 18).foregroundStyle(Theme.accent)
                Text(screenLine).font(Theme.sans(15)).foregroundStyle(Theme.text2)
            }
            .modifier(rise(3))
            actions
                .padding(.top, roomy ? 22 : 10)
                .modifier(rise(4))
            Text(host.settings.profile?.hint ?? String(localized: "Personnalisé : vos propres réglages, dans Réglages."))
                .font(Theme.sans(13.5))
                .foregroundStyle(Theme.muted)
                .modifier(rise(4))
            if picture?.source != .lastSession, roomy {
                ImageChain(source: picture?.source, fetching: fetching, neverOpened: neverOpened)
                    .padding(.top, 10)
                    .modifier(rise(5))
            }
        }
        .frame(maxWidth: 820, alignment: .leading)
    }

    @ViewBuilder private var actions: some View {
        HStack(spacing: 14) {
            if online == false {
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
                        Text(neverOpened ? "Ouvrir pour la première fois" : "Se connecter")
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
            Menu {
                Button("Réglages de \(host.name)") { app.route = .settings(host.id) }
                Divider()
                Button("Oublier cet ordinateur", role: .destructive) { app.store.remove(host) }
            } label: {
                Image(systemName: "ellipsis").foregroundStyle(Theme.text2)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .frame(width: 40, height: 40)
            .glass(10)
        }
    }
}

/// D'où vient l'image affichée : dernière session, aperçu, signature.
private struct ImageChain: View {
    let source: HostImagery.Source?
    let fetching: Bool
    let neverOpened: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var pulse = false

    var body: some View {
        HStack(spacing: 16) {
            Theme.eyebrow(String(localized: "Image affichée"))
            step(neverOpened ? String(localized: "Dernière session, aucune") : String(localized: "Dernière session, aucune gardée"), state: .absent)
            rule
            step(fetching && source == nil ? String(localized: "Aperçu, en cours…") : source == .preview ? String(localized: "Aperçu demandé à la Station") : String(localized: "Aperçu"), state: source == .preview ? .on : fetching ? .loading : .absent)
            rule
            step(String(localized: "Signature"), state: source == nil ? .on : .absent)
        }
        .padding(.horizontal, 16)
        .frame(height: 44)
        .glass(12)
        .onAppear {
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 0.6).repeatForever(autoreverses: true)) { pulse = true }
        }
    }

    private enum State { case on, loading, absent }

    private var rule: some View { Rectangle().fill(Color.white.opacity(0.15)).frame(width: 18, height: 1) }

    private func step(_ title: String, state: State) -> some View {
        HStack(spacing: 8) {
            Circle()
                .fill(state == .on ? Theme.neonCore : state == .loading ? Theme.text2 : .clear)
                .overlay(Circle().strokeBorder(state == .absent ? Theme.ghost : .clear))
                .frame(width: 7, height: 7)
                .neon(state == .on ? .lit : .off)
                .opacity(state == .loading && pulse ? 0.35 : 1)
            Text(title).font(Theme.sans(13)).foregroundStyle(state == .absent ? Theme.ghost : Theme.text)
        }
    }
}

/// Latence de la dernière session, lue contre la durée d'une image.
private struct LastSessionPanel: View {
    let median: Double
    let p95: Double?
    let duration: TimeInterval?

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline) {
                Theme.eyebrow(String(localized: "Dernière session"))
                Spacer()
                if let duration { Text(Moment.duration(duration)).font(Theme.sans(12.5)).foregroundStyle(Theme.muted) }
            }
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(Moment.ms(median, unit: false)).font(Theme.mono(30, .medium)).foregroundStyle(Theme.text)
                Text("ms médiane").font(Theme.mono(12)).foregroundStyle(Theme.muted)
            }
            FrameRuler(median: median, p95: p95)
            if let p95 {
                Text("95 % des images sous \(Moment.ms(p95))").font(Theme.sans(12)).foregroundStyle(Theme.faint)
            }
        }
        .padding(.horizontal, 18).padding(.vertical, 16)
        .glass(14)
    }
}

/// Aucun ordinateur pour l'instant.
private struct Welcome: View {
    @EnvironmentObject var app: AppModel
    let arrived: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Theme.eyebrow(String(localized: "Bienvenue"), color: Theme.muted).modifier(Rise(arrived: arrived, delay: 0.15))
            Text("Vos ordinateurs,\nici.")
                .font(Theme.display(96, .bold)).tracking(-96 * 0.045)
                .foregroundStyle(Theme.text)
                .modifier(Rise(arrived: arrived, delay: 0.25))
            Text("Ajoutez un ordinateur équipé de KyberNow Station, qu'il soit sous Windows, macOS ou Linux. Sur ce réseau, il apparaît tout seul en bas de l'écran.")
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
    @Binding var selectedID: UUID?
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

    private var count: Int { others.count + newcomers.count + 1 }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline) {
                Theme.eyebrow(featured == nil ? String(localized: "Sur ce réseau") : String(localized: "Vos autres ordinateurs"), color: Theme.muted)
                Spacer()
                Text("Windows, macOS ou Linux : ils s'ouvrent tous de la même façon.")
                    .font(Theme.sans(12.5)).foregroundStyle(Theme.faint)
            }
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 16) {
                    ForEach(Array(others.enumerated()), id: \.element.id) { i, host in
                        HostCard(host: host, picture: imagery.pictures[host.id]) {
                            selectedID = host.id
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

private struct HostCard: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let picture: HostImagery.Picture?
    let select: () -> Void
    @State private var hovering = false

    private var status: HostStatus? { app.store.status[host.id] }

    private var line: String {
        switch status?.online {
        case nil: return String(localized: "Recherche…")
        case false?: return String(localized: "Hors ligne")
        case true?:
            let parts = [status?.info?.os, host.lastLatencyMs.map { Moment.ms($0) }].compactMap { $0 }
            return parts.isEmpty ? String(localized: "En ligne") : parts.joined(separator: " · ")
        }
    }

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
                        Text(line).font(Theme.sans(12)).foregroundStyle(Theme.muted).lineLimit(1)
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
            Button("Réglages…") { app.route = .settings(host.id) }
            Divider()
            Button("Oublier cet ordinateur", role: .destructive) { app.store.remove(host) }
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
                    Text("TROUVÉ SUR LE RÉSEAU").font(Theme.mono(10, .medium)).tracking(0.8)
                }
                .foregroundStyle(Theme.accentLight)
                Text(found.name).font(Theme.display(22, .semibold)).tracking(-0.4).foregroundStyle(Theme.text).lineLimit(1)
                Text(candidate.address).font(Theme.mono(11)).foregroundStyle(Theme.muted)
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
            VStack(alignment: .leading) {
                HStack(spacing: 8) {
                    Icon("plus", size: 16, bold: true).foregroundStyle(Theme.accent)
                    Text("Ajouter un ordinateur").font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
                }
                Spacer()
                VStack(alignment: .leading, spacing: 6) {
                    Text("QR code d'appairage")
                    Text("Lien kybernow://")
                    Text("Adresse et port")
                }
                .font(Theme.sans(12.5))
                .foregroundStyle(Theme.muted)
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
