// Accueil (maquette « 1 · Accueil : la scène de l'ordinateur », docs/link-ui.md) :
// l'ordinateur choisi occupe tout l'écran (sa dernière image en fond) avec son
// état, son nom, son système, l'écran que la session demandera, « Se
// connecter », les profils et l'accès à ses réglages ; rien de technique (les
// chiffres sont dans les Statistiques). En haut, la ligne des manettes et les
// réglages de l'app ; en bas, une rangée de cartes pour passer d'un
// ordinateur à l'autre (le focus d'une carte change la scène).

import KyberKit
import SwiftUI

struct HomeView: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @EnvironmentObject private var pads: GamepadBridge
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    enum Focus: Hashable { case connect, profile(Profile), settings, pads, host(UUID), found(String), add, join, appSettings }
    @FocusState private var focus: Focus?
    @State private var pulse = false

    private var host: Host? { app.selected.flatMap(store.host) ?? store.ordered.first }
    /// En ligne, hors ligne, ou nil tant qu'il n'a pas répondu (« Recherche… »).
    private func online(_ h: Host) -> Bool? { store.status[h.id]?.online }

    var body: some View {
        ZStack(alignment: .topLeading) {
            // Le décor : l'image de l'ordinateur choisi.
            ZStack {
                if let host {
                    HostBackdrop(host: host, online: online(host) != false)
                        .id(host.id)
                        .transition(.opacity.combined(with: .blurReplace))
                } else {
                    Signature(seed: "KyberNow")
                }
            }
            .animation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.7), value: host?.id)

            topBar
                .padding(.leading, 96).padding(.top, 56)
                .frame(width: 1920, alignment: .leading)
            hero
                .frame(width: 1240, alignment: .leading)
                .padding(.leading, 96).padding(.top, 176)
            shelf
                .frame(width: 1920 - 192)
                .padding(.leading, 96).padding(.top, 1080 - 56 - 216 - 20 - 30)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .defaultFocus($focus, store.hosts.isEmpty ? .add : .connect)
        .onChange(of: focus) { _, new in
            if case let .host(id)? = new, app.selected != id {
                app.selected = id
            }
        }
        .onAppear {
            if app.selected == nil { app.selected = store.ordered.first?.id }
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 1.2).repeatForever(autoreverses: true)) { pulse = true }
        }
    }

    // MARK: Haut : marque, manettes, réglages de l'app

    private var topBar: some View {
        HStack(spacing: 20) {
            Mark(size: 34)
            Wordmark(size: 28)
            Spacer(minLength: 0)
            padsLine
            Button { app.screen = .appSettings } label: {
                HStack(spacing: 12) {
                    Icon("sliders-horizontal", size: 28).foregroundStyle(Theme.accent)
                    Text("Réglages de l'app").font(Theme.sans(22, .medium))
                }
                .padding(.leading, 20).padding(.trailing, 24)
                .frame(height: 60)
            }
            .buttonStyle(GlassButton(radius: 30))
            .focused($focus, equals: .appSettings)
        }
        .frame(width: 1920 - 192, height: 60)
        // Monter depuis n'importe où atteint la barre.
        .focusSection()
    }

    // MARK: L'ordinateur choisi

    @ViewBuilder private var hero: some View {
        if let host {
            let state = online(host)
            let problem = store.status[host.id]?.problem
            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: 12) {
                    statusDot(state)
                    Text(LinkText.status(state)).font(Theme.sans(21))
                }
                .padding(.leading, 16).padding(.trailing, 18)
                .frame(height: 44)
                .glass(22)
                // Une ligne, interligne de 0,9 comme la maquette : un nom long rétrécit.
                Text(host.name)
                    .font(Theme.display(140, 680))
                    .tracking(-140 * 0.045)
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                    .padding(.vertical, -16)
                    .frame(maxWidth: 1080, alignment: .leading)
                if let os = store.status[host.id]?.info?.os {
                    Text(os).font(Theme.sans(24)).foregroundStyle(Theme.muted)
                }
                if let problem {
                    HStack(spacing: 12) {
                        Icon("warning", size: 26).foregroundStyle(Theme.warn)
                        Text(LinkText.problem(problem)).font(Theme.sans(22)).foregroundStyle(Theme.warnText)
                    }
                }
                HStack(spacing: 14) {
                    Icon("monitor", size: 28).foregroundStyle(Theme.accent)
                    Text(screenLine(host)).font(Theme.sans(24)).foregroundStyle(Theme.text2)
                }
                HStack(spacing: 24) {
                    Button { primaryAction(host, state: state) } label: {
                        HStack(spacing: 16) {
                            Text(primaryLabel(host, state: state))
                            Icon(state != false ? "arrow-right" : store.canWake(host) ? "power" : "arrows-clockwise", size: 28, bold: true)
                        }
                    }
                    .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
                    .focused($focus, equals: .connect)
                    HStack(spacing: 6) {
                        ForEach(Profile.allCases) { p in
                            Button(p.label) { select(p, for: host) }
                                .buttonStyle(ChoiceButton(selected: host.settings.profile == p, height: 64))
                                .focused($focus, equals: .profile(p))
                        }
                    }
                    .padding(6)
                    .glass(16)
                    .focusSection()
                    Button { app.showSettings(host.id) } label: {
                        HStack(spacing: 12) {
                            Icon("sliders-horizontal", size: 26)
                            Text("Réglages").font(Theme.sans(22, .medium))
                        }
                        .padding(.leading, 20).padding(.trailing, 24)
                        .frame(height: 64)
                    }
                    .buttonStyle(GlassButton(radius: 18))
                    .focused($focus, equals: .settings)
                    .accessibilityLabel(LinkText.settingsTitle(host.name))
                }
                .padding(.top, 26)
                Text(Profile.hint(host.settings.profile))
                    .font(Theme.sans(22))
                    .foregroundStyle(Theme.muted)
            }
        } else {
            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: 14) {
                    Circle().fill(Theme.accent).frame(width: 9, height: 9)
                    Theme.eyebrow(String(localized: "Bienvenue")).font(Theme.mono(18))
                }
                Text("Vos ordinateurs, sur ce téléviseur")
                    .font(Theme.display(110, 680))
                    .tracking(-110 * 0.045)
                    .foregroundStyle(Theme.text)
                    .frame(width: 1080, alignment: .leading)
                Text("Ajoutez un ordinateur équipé de KyberNow Station : depuis votre Mac par iCloud, sur ce réseau, ou par un code à accepter sur l'ordinateur.")
                    .font(Theme.sans(24)).foregroundStyle(Theme.text2)
                    .frame(width: 900, alignment: .leading)
            }
        }
    }

    /// La ligne des manettes (apps de salon) : « 2 manettes » ; un clic ouvre leur test.
    private var padsLine: some View {
        Button { app.showGamepads(from: .home) } label: {
            HStack(spacing: 12) {
                Icon("game-controller", size: 28).foregroundStyle(pads.pads.isEmpty ? Theme.faint : Theme.accent)
                Text(LinkText.controllers(pads.pads.count)).font(Theme.sans(22, .medium))
            }
            .padding(.leading, 20).padding(.trailing, 24)
            .frame(height: 60)
        }
        .buttonStyle(GlassButton(radius: 30))
        .focused($focus, equals: .pads)
    }

    private func statusDot(_ on: Bool?) -> some View {
        Group {
            switch on {
            case true?:
                Circle().fill(Theme.ok).frame(width: 10, height: 10)
                    .background(Circle().fill(Theme.ok.opacity(pulse ? 0 : 0.25)).frame(width: pulse ? 26 : 18, height: pulse ? 26 : 18))
            case nil:
                Circle().fill(Theme.text2).frame(width: 10, height: 10).opacity(pulse ? 0.35 : 1)
            case false?:
                Circle().strokeBorder(Theme.faint, lineWidth: 1.5).frame(width: 10, height: 10)
            }
        }
        .frame(width: 10, height: 10)
    }

    // MARK: Vos ordinateurs

    private var shelf: some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack {
                Theme.eyebrow(String(localized: "Vos ordinateurs"))
                Spacer()
                HStack(spacing: 30) {
                    HStack(spacing: 10) {
                        ZStack {
                            Circle().strokeBorder(Theme.muted, lineWidth: 1.6)
                            HStack(spacing: 8) {
                                Image(systemName: "arrowtriangle.left.fill").font(.system(size: 9))
                                Image(systemName: "arrowtriangle.right.fill").font(.system(size: 9))
                            }
                            .foregroundStyle(Theme.text)
                        }
                        .frame(width: 30, height: 30)
                        Text("Changer d'ordinateur")
                    }
                    .font(Theme.sans(19))
                    .foregroundStyle(Theme.muted)
                    GlyphHints()
                }
            }
            HStack(spacing: 24) {
                ForEach(store.ordered.prefix(4)) { h in
                    hostCard(h)
                }
                // La rangée tient six cartes : l'annonce du réseau laisse sa place à « Rejoindre avec un code ».
                if let found = app.newOnNetwork.first, store.ordered.count < 4 {
                    foundCard(found)
                }
                addCard
                joinCard
                Spacer(minLength: 0)
            }
            .focusSection()
        }
    }

    private static let cardWidth: CGFloat = (1920 - 192 - 5 * 24) / 6

    private func hostCard(_ h: Host) -> some View {
        let state = online(h)
        return Button {
            if state == true { app.connect(h.id) } else { app.selected = h.id }
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                HostPicture(host: h, online: state != false)
                    .frame(width: Self.cardWidth, height: 124)
                    .clipped()
                VStack(alignment: .leading, spacing: 6) {
                    Text(h.name).font(Theme.sans(23, .semibold)).foregroundStyle(state == false ? Theme.muted : Theme.text).lineLimit(1)
                    HStack(spacing: 9) {
                        if state == true {
                            Circle().fill(Theme.ok).frame(width: 9, height: 9)
                        } else {
                            Circle().strokeBorder(Theme.faint, lineWidth: 1.5).frame(width: 9, height: 9)
                        }
                        Text(LinkText.status(state)).font(Theme.sans(17)).foregroundStyle(Theme.muted).lineLimit(1)
                    }
                }
                .padding(.horizontal, 18).padding(.vertical, 14)
                Spacer(minLength: 0)
            }
            .frame(width: Self.cardWidth, height: 216, alignment: .topLeading)
            .overlay(alignment: .bottom) {
                if h.id == host?.id {
                    UnevenRoundedRectangle(topLeadingRadius: 2, topTrailingRadius: 2)
                        .fill(Theme.accent)
                        .frame(height: 4)
                        .padding(.horizontal, 18)
                        .shadow(color: Theme.accentFill, radius: 6)
                }
            }
        }
        .buttonStyle(GlassButton(radius: 18))
        .focused($focus, equals: .host(h.id))
        .accessibilityLabel("\(h.name), \(LinkText.status(state))")
    }

    private func foundCard(_ found: Discovery.Found) -> some View {
        Button { app.screen = .addHost } label: {
            VStack(alignment: .leading) {
                HStack(spacing: 9) {
                    Icon("wifi-high", size: 20, bold: true)
                    Text(String(localized: "Sur le réseau").uppercased()).font(Theme.mono(14, .medium)).tracking(1.1)
                }
                .foregroundStyle(Theme.accentLight)
                Spacer()
                Text(found.name).font(Theme.display(32, 640)).tracking(-0.6).lineLimit(2).minimumScaleFactor(0.55)
                Spacer()
                Text("Ajouter").font(Theme.sans(19, .semibold)).foregroundStyle(Theme.accentLight)
            }
            .padding(20)
            .frame(width: Self.cardWidth, height: 216, alignment: .leading)
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .strokeBorder(Theme.accent.opacity(pulse ? 0.45 : 0.75), lineWidth: 2)
                    .shadow(color: Theme.accentFill.opacity(pulse ? 0.2 : 0.35), radius: pulse ? 12 : 22)
            )
        }
        .buttonStyle(GlassButton(radius: 18, fill: Theme.accentFill.opacity(0.14)))
        .focused($focus, equals: .found(found.id))
    }

    private var addCard: some View {
        Button { app.screen = .addHost } label: {
            VStack(alignment: .leading) {
                Icon("plus", size: 40).foregroundStyle(Theme.accent)
                Spacer()
                VStack(alignment: .leading, spacing: 6) {
                    Text("Ajouter un ordinateur").font(Theme.sans(23, .semibold))
                    Text("iCloud, réseau, code ou adresse").font(Theme.sans(17)).foregroundStyle(Theme.muted)
                }
            }
            .padding(20)
            .frame(width: Self.cardWidth, height: 216, alignment: .leading)
            .background(Stripes().opacity(0.03))
        }
        .buttonStyle(GlassButton(radius: 18, fill: Theme.background.opacity(0.5)))
        .focused($focus, equals: .add)
    }

    /// « Rejoindre avec un code » (KyberNow Share), à côté de l'ajout d'un ordinateur.
    private var joinCard: some View {
        Button { app.screen = .join(nil) } label: {
            VStack(alignment: .leading) {
                Icon("share-network", size: 40).foregroundStyle(Theme.accent)
                Spacer()
                Text(ShareText.join).font(Theme.sans(23, .semibold))
            }
            .padding(20)
            .frame(width: Self.cardWidth, height: 216, alignment: .leading)
            .background(Stripes().opacity(0.03))
        }
        .buttonStyle(GlassButton(radius: 18, fill: Theme.background.opacity(0.5)))
        .focused($focus, equals: .join)
    }

    // MARK: Bouton principal

    /// En ligne : « Se connecter » ; hors ligne : « Réveiller l'ordinateur »
    /// quand l'app connaît une de ses cartes réseau, sinon « Réessayer » ;
    /// pendant le réveil : « Réveil de l'ordinateur… », sans effet (le focus reste).
    private func primaryLabel(_ host: Host, state: Bool?) -> String {
        guard state == false else { return String(localized: "Se connecter") }
        if store.waking.contains(host.id) { return LinkText.waking }
        return store.canWake(host) ? LinkText.wake : String(localized: "Réessayer")
    }

    private func primaryAction(_ host: Host, state: Bool?) {
        guard state == false else { app.connect(host.id); return }
        guard !store.waking.contains(host.id) else { return }
        if store.canWake(host) { store.wake(host) } else { Task { await store.refresh(host) } }
    }

    // MARK: Textes

    private func select(_ profile: Profile, for host: Host) {
        var h = host
        h.settings.apply(profile)
        store.update(h)
    }

    private func screenLine(_ h: Host) -> String {
        Self.screenLine(h, info: store.status[h.id]?.info)
    }

    /// L'écran que la session demandera (même calcul que la session).
    static func screenLine(_ h: Host, info: HostInfo?) -> String {
        let s = h.settings
        let m = TVDisplay.mode(for: s)
        let tv = TVDisplay.native
        let virtual = s.display != .physical && (info?.virtualDisplay ?? true)
        return LinkText.screen(width: m.width, height: m.height, refresh: m.refresh, virtual: virtual,
                               sizedForThisScreen: m.width == tv.width && m.height == tv.height)
    }

    static func short(_ name: String) -> String {
        name.replacingOccurrences(of: " Controller", with: "")
    }

    /// Virgule décimale en français, point en anglais.
    static func ms(_ v: Double) -> String {
        LinkFormat.decimal(v)
    }
}

/// Rayures fines à 135° (carte « Ajouter un ordinateur »).
struct Stripes: View {
    var body: some View {
        Canvas { ctx, size in
            var x: CGFloat = -size.height
            while x < size.width {
                var p = Path()
                p.move(to: CGPoint(x: x, y: size.height))
                p.addLine(to: CGPoint(x: x + size.height, y: 0))
                ctx.stroke(p, with: .color(.white), lineWidth: 1)
                x += 13
            }
        }
        .accessibilityHidden(true)
    }
}
