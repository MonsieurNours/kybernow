// Accueil (maquette « 1 · Accueil : la scène de l'ordinateur ») : l'ordinateur
// choisi occupe tout l'écran (sa dernière image en fond), une rangée de
// cartes en bas pour passer de l'un à l'autre ; le focus d'une carte change
// la scène.

import KyberKit
import SwiftUI

struct HomeView: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @EnvironmentObject private var pads: GamepadBridge
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    enum Focus: Hashable { case connect, profile(Profile), host(UUID), found(String), add, gamepads, settings }
    @FocusState private var focus: Focus?
    @State private var pulse = false

    private var host: Host? { app.selected.flatMap(store.host) ?? store.ordered.first }
    private func online(_ h: Host) -> Bool { store.status[h.id]?.online ?? false }

    var body: some View {
        ZStack(alignment: .topLeading) {
            // Le décor : l'image de l'ordinateur choisi.
            ZStack {
                if let host {
                    HostBackdrop(host: host, online: online(host))
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
                .frame(width: 1080, alignment: .leading)
                .padding(.leading, 96).padding(.top, 196)
            if host != nil {
                readyCard
                    .frame(width: 460)
                    .padding(.leading, 1920 - 96 - 460).padding(.top, 300)
            }
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

    // MARK: Haut : marque, manettes, profil, réglages

    private var topBar: some View {
        HStack(spacing: 20) {
            Mark(size: 34)
            Wordmark(size: 28)
            Spacer(minLength: 0)
            Button { app.screen = .gamepads } label: {
                HStack(spacing: 12) {
                    Icon("game-controller", size: 28).foregroundStyle(Theme.accent)
                    Text(padsLabel).font(Theme.sans(22, .medium))
                }
                .padding(.leading, 20).padding(.trailing, 24)
                .frame(height: 60)
            }
            .buttonStyle(GlassButton(radius: 30))
            .focused($focus, equals: .gamepads)
            HStack(spacing: 12) {
                Circle()
                    .fill(LinearGradient(colors: [Theme.accentFill, Theme.accent], startPoint: .topLeading, endPoint: .bottomTrailing))
                    .frame(width: 44, height: 44)
                    .overlay(Icon("user", size: 24, bold: true).foregroundStyle(.white))
                Text("Profil").font(Theme.sans(22, .medium))
            }
            .padding(.leading, 8).padding(.trailing, 24)
            .frame(height: 60)
            .glass(30)
            .accessibilityElement(children: .combine)
            .accessibilityLabel("Profil tvOS en cours : ses ordinateurs et ses réglages")
            Button { if let host { app.screen = .settings(host.id) } } label: {
                Icon("sliders-horizontal", size: 28).frame(width: 60, height: 60)
            }
            .buttonStyle(GlassButton(radius: 30))
            .focused($focus, equals: .settings)
            .disabled(host == nil)
            .accessibilityLabel("Réglages")
        }
        .frame(width: 1920 - 192, height: 60)
        // Monter depuis n'importe où atteint la barre, jusqu'aux réglages à droite.
        .focusSection()
    }

    private var padsLabel: String {
        switch pads.pads.count {
        case 0: return "Manettes"
        case 1: return "1 manette"
        default: return "\(pads.pads.count) manettes"
        }
    }

    // MARK: L'ordinateur choisi

    @ViewBuilder private var hero: some View {
        if let host {
            let isOnline = online(host)
            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: 14) {
                    Circle().fill(Theme.accent).frame(width: 9, height: 9)
                    Theme.eyebrow(caption(host), color: Theme.muted).font(Theme.mono(18))
                }
                // Une ligne, interligne de 0,9 comme la maquette : un nom long rétrécit.
                Text(host.name)
                    .font(Theme.display(140, 680))
                    .tracking(-140 * 0.045)
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                    .padding(.vertical, -16)
                HStack(spacing: 20) {
                    HStack(spacing: 12) {
                        statusDot(isOnline)
                        Text(status(host)).font(Theme.sans(21))
                    }
                    .padding(.leading, 16).padding(.trailing, 18)
                    .frame(height: 44)
                    .glass(22)
                    if specs(host).isEmpty == false {
                        Text(specs(host)).font(Theme.mono(18)).foregroundStyle(Theme.muted)
                    }
                }
                .padding(.top, 4)
                HStack(spacing: 14) {
                    Icon("monitor", size: 28).foregroundStyle(Theme.accent)
                    Text(screenLine(host, online: isOnline)).font(Theme.sans(24)).foregroundStyle(Theme.text2)
                }
                HStack(spacing: 28) {
                    Button {
                        if isOnline { app.connect(host.id) } else { Task { await store.refresh(host) } }
                    } label: {
                        HStack(spacing: 16) {
                            Text(isOnline ? "Se connecter" : "Réessayer")
                            Icon("arrow-right", size: 28, bold: true)
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
                }
                .padding(.top, 34)
                Text(hint(host.settings.profile))
                    .font(Theme.sans(22))
                    .foregroundStyle(Theme.muted)
            }
        } else {
            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: 14) {
                    Circle().fill(Theme.accent).frame(width: 9, height: 9)
                    Theme.eyebrow("Bienvenue").font(Theme.mono(18))
                }
                Text("Vos ordinateurs, sur ce téléviseur")
                    .font(Theme.display(110, 680))
                    .tracking(-110 * 0.045)
                    .foregroundStyle(Theme.text)
                Text("Ajoutez un ordinateur équipé de KyberNow : depuis votre Mac par iCloud, sur ce réseau, ou par un code à accepter sur l'ordinateur.")
                    .font(Theme.sans(24)).foregroundStyle(Theme.text2)
                    .frame(width: 900, alignment: .leading)
            }
        }
    }

    private func statusDot(_ on: Bool) -> some View {
        Group {
            if on {
                Circle().fill(Theme.ok).frame(width: 10, height: 10)
                    .background(Circle().fill(Theme.ok.opacity(pulse ? 0 : 0.25)).frame(width: pulse ? 26 : 18, height: pulse ? 26 : 18))
            } else {
                Circle().strokeBorder(Theme.faint, lineWidth: 1.5).frame(width: 10, height: 10)
            }
        }
        .frame(width: 10, height: 10)
    }

    // MARK: Prêt à jouer

    private var readyCard: some View {
        let route = AudioRoute.current
        let mode = host.map { TVDisplay.mode(for: $0.settings) } ?? TVDisplay.native
        return VStack(alignment: .leading, spacing: 22) {
            Theme.eyebrow("Prêt à jouer")
            readyRow("game-controller", padsTitle, padsDetail)
            readyRow(route.isLate ? "warning" : "speaker-high",
                     "Son \(route.channelsLabel) · \(route.name)",
                     route.kind == .hdmi ? "LPCM \(route.channels) canaux · sortie \(route.latencyMs) ms" : "Sortie \(route.latencyMs) ms")
            readyRow("monitor", "\(TVDisplay.label(mode.width, mode.height)) · \(mode.refresh) Hz · \(host?.settings.codec == .h264 ? "H.264" : "HEVC")",
                     host?.lastLatencyMs.map { "Dernière session \(Self.ms($0)) ms médiane" } ?? "Pas encore de session ici")
        }
        .padding(.horizontal, 30).padding(.vertical, 28)
        .glass(24)
    }

    private var padsTitle: String {
        guard !pads.pads.isEmpty else { return "Aucune manette" }
        return pads.pads.prefix(2).map { "J\($0.player) \(Self.short($0.name))" }.joined(separator: " · ")
    }

    private var padsDetail: String {
        let levels = pads.pads.prefix(2).compactMap { $0.battery }.map { "\(Int(($0 * 100).rounded())) %" }
        if pads.pads.isEmpty { return "Allumez-en une en mode appairage" }
        return levels.isEmpty ? (pads.pads.count > 1 ? "Branchées" : "Branchée") : "\(levels.count > 1 ? "Batteries" : "Batterie") \(levels.joined(separator: " · "))"
    }

    private func readyRow(_ icon: String, _ title: String, _ detail: String) -> some View {
        HStack(spacing: 16) {
            Icon(icon, size: 30).foregroundStyle(Theme.text)
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(Theme.sans(22, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                Text(detail).font(Theme.mono(15)).foregroundStyle(Theme.muted).lineLimit(1)
            }
            Spacer(minLength: 0)
        }
    }

    // MARK: Vos ordinateurs

    private var shelf: some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack {
                Theme.eyebrow("Vos ordinateurs")
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
                if let found = app.newOnNetwork.first, store.ordered.count < 5 {
                    foundCard(found)
                }
                addCard
                Spacer(minLength: 0)
            }
            .focusSection()
        }
    }

    private static let cardWidth: CGFloat = (1920 - 192 - 5 * 24) / 6

    private func hostCard(_ h: Host) -> some View {
        let on = online(h)
        return Button {
            if on { app.connect(h.id) } else { app.selected = h.id }
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                HostPicture(host: h, online: on)
                    .frame(width: Self.cardWidth, height: 124)
                    .clipped()
                VStack(alignment: .leading, spacing: 6) {
                    Text(h.name).font(Theme.sans(23, .semibold)).foregroundStyle(on ? Theme.text : Theme.muted).lineLimit(1)
                    HStack(spacing: 9) {
                        if on { Circle().fill(Theme.ok).frame(width: 9, height: 9) } else { Circle().strokeBorder(Theme.faint, lineWidth: 1.5).frame(width: 9, height: 9) }
                        Text(shortStatus(h)).font(Theme.sans(17)).foregroundStyle(Theme.muted).lineLimit(1)
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
        .accessibilityLabel("\(h.name), \(shortStatus(h))")
    }

    private func foundCard(_ found: Discovery.Found) -> some View {
        Button { app.screen = .addHost } label: {
            VStack(alignment: .leading) {
                HStack(spacing: 9) {
                    Icon("wifi-high", size: 20, bold: true)
                    Text("SUR LE RÉSEAU").font(Theme.mono(14, .medium)).tracking(1.1)
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

    // MARK: Textes

    private func select(_ profile: Profile, for host: Host) {
        var h = host
        h.settings.apply(profile)
        store.update(h)
    }

    private func hint(_ p: Profile?) -> String {
        switch p {
        case .jeu?: return "Jeu : latence minimale, 4K à 60 Hz, manettes prêtes."
        case .bureau?: return "Bureau : texte net (4:4:4), aucune image perdue. La télécommande sert de souris."
        case .max?: return "Max : sans compromis, 4:4:4 et gros débit. Idéal en réseau local filaire."
        case nil: return "Personnalisé : vos réglages pour cet ordinateur."
        }
    }

    private func caption(_ h: Host) -> String {
        if let p = app.store.imagery.pictures[h.id] {
            return "\(p.source == .lastSession ? "Dernière image" : "Aperçu") · \(Self.when(p.date))"
        }
        if let last = h.lastSession { return "Dernière session · \(Self.when(last))" }
        return "Jamais ouvert sur cette Apple TV"
    }

    private func status(_ h: Host) -> String {
        guard online(h) else { return "Hors ligne" }
        return app.isLocal(h) ? "En ligne · réseau local" : "En ligne · par VPN"
    }

    private func shortStatus(_ h: Host) -> String {
        guard online(h) else {
            if let last = h.lastSession { return "Vu \(Self.ago(last))" }
            return "Hors ligne"
        }
        return app.isLocal(h) ? "Réseau local" : "VPN"
    }

    private func specs(_ h: Host) -> String {
        let info = store.status[h.id]?.info
        var parts: [String] = []
        if let os = info?.os { parts.append(os.replacingOccurrences(of: " Pro", with: "").components(separatedBy: " ").prefix(2).joined(separator: " ")) }
        if let gpu = info?.gpu { parts.append(Self.gpu(gpu)) }
        parts.append(h.address)
        return parts.joined(separator: " · ")
    }

    private func screenLine(_ h: Host, online: Bool) -> String {
        guard online else { return "Allumez-le ou sortez-le de veille : il réapparaîtra tout seul." }
        let m = TVDisplay.mode(for: h.settings)
        if store.status[h.id]?.info?.virtualDisplay ?? true {
            let tv = TVDisplay.native
            let same = m.width == tv.width && m.height == tv.height
            return "Un écran de \(m.width) × \(m.height) à \(m.refresh) Hz l'attend\(same ? ", celui de ce téléviseur" : "")."
        }
        return "Son écran, jusqu'à \(m.width) × \(m.height) à \(m.refresh) Hz."
    }

    static func gpu(_ name: String) -> String {
        name.replacingOccurrences(of: "NVIDIA GeForce ", with: "")
            .replacingOccurrences(of: "NVIDIA ", with: "")
            .replacingOccurrences(of: "AMD Radeon ", with: "RX ")
            .replacingOccurrences(of: "Intel(R) ", with: "")
    }

    static func short(_ name: String) -> String {
        name.replacingOccurrences(of: " Controller", with: "")
    }

    static func ms(_ v: Double) -> String {
        String(format: "%.1f", v).replacingOccurrences(of: ".", with: ",")
    }

    /// « aujourd'hui, 10 h 12 », « hier, 21 h 40 », « lundi, 18 h 05 », « il y a 12 jours ».
    static func when(_ date: Date) -> String {
        let cal = Calendar.current
        let hm = date.formatted(.dateTime.hour(.defaultDigits(amPM: .omitted)).minute(.twoDigits).locale(Locale(identifier: "fr_FR")))
            .replacingOccurrences(of: ":", with: " h ")
        if cal.isDateInToday(date) { return "aujourd'hui, \(hm)" }
        if cal.isDateInYesterday(date) { return "hier, \(hm)" }
        let days = cal.dateComponents([.day], from: cal.startOfDay(for: date), to: cal.startOfDay(for: Date())).day ?? 0
        if days < 7 { return "\(date.formatted(.dateTime.weekday(.wide).locale(Locale(identifier: "fr_FR")))), \(hm)" }
        return ago(date)
    }

    static func ago(_ date: Date) -> String {
        let days = Calendar.current.dateComponents([.day], from: date, to: Date()).day ?? 0
        if days < 1 { return "aujourd'hui" }
        if days == 1 { return "hier" }
        return "il y a \(days) jours"
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
