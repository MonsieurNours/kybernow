// Pièces de l'accueil (maquettes iOS, planche 1) : le décor, le bouton
// principal et son pan coupé, le choix du profil, les cartes de la rangée
// « Vos ordinateurs », la colonne « Prêt à jouer » de l'iPad, et ce que
// l'appareil voit de ses périphériques (clavier, trackpad, sortie du son).

import AVFoundation
import GameController
import KyberKit
import SwiftUI

/// Courbe de la charte, pour les arrivées et les vols de l'accueil.
enum HomeCurve {
    static func rise(_ duration: Double = 0.7) -> Animation { .timingCurve(0.2, 0.9, 0.1, 1, duration: duration) }
}

// MARK: - Décor

/// La dernière image de l'ordinateur choisi, en plein écran, qui dérive
/// lentement ; un autre ordinateur la remplace en fondu (avec un flou qui se
/// dissipe, comme sur l'Apple TV). Sans ordinateur : la signature de KyberNow.
struct HomeBackdrop: View {
    let host: Host?
    var online = true
    /// iPad pleine largeur : le voile de l'Apple TV (à gauche et en bas) ;
    /// sinon celui du portrait, en bas seulement, sous le texte.
    var wide = false

    var body: some View {
        ZStack {
            Theme.background
            ZStack {
                if let host {
                    Group {
                        if wide {
                            HostBackdrop(host: host, online: online)
                        } else {
                            HomePortraitBackdrop(host: host, online: online)
                        }
                    }
                    .id(host.id)
                    .transition(.opacity.combined(with: .blurReplace))
                } else {
                    Signature(seed: "KyberNow")
                        .transition(.opacity)
                }
            }
            .animation(HomeCurve.rise(0.6), value: host?.id)
        }
        .accessibilityHidden(true)
    }
}

/// Portrait (iPhone, iPad en Split View) : l'image dérive, voilée en bas
/// pour le texte et en haut pour la barre d'état.
private struct HomePortraitBackdrop: View {
    let host: Host
    let online: Bool
    @State private var drift = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            // Le cadre vient de l'écran, pas de l'image (remplie et rognée).
            Color.clear
                .overlay {
                    HostPicture(host: host, online: online)
                        .scaleEffect(drift ? 1.06 : 1, anchor: UnitPoint(x: 0.6, y: 0.5))
                        .offset(x: drift ? -8 : 0, y: drift ? -6 : 0)
                }
                .clipped()
            LinearGradient(stops: [.init(color: Theme.background, location: 0),
                                   .init(color: Theme.background.opacity(0.94), location: 0.30),
                                   .init(color: Theme.background.opacity(0.6), location: 0.50),
                                   .init(color: Theme.background.opacity(0), location: 0.68)],
                           startPoint: .bottom, endPoint: .top)
            LinearGradient(colors: [Theme.background.opacity(0.7), Theme.background.opacity(0)], startPoint: .top, endPoint: .bottom)
                .frame(height: 150)
                .frame(maxHeight: .infinity, alignment: .top)
        }
        .onAppear {
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 40).repeatForever(autoreverses: true)) { drift = true }
        }
    }
}

// MARK: - Arrivée en cascade

/// Les blocs de l'accueil montent en fondu, l'un après l'autre, quand
/// l'accueil arrive après le lancement.
struct HomeRise: ViewModifier {
    var shown: Bool
    var delay: Double

    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .offset(y: shown ? 0 : 18)
            .animation(HomeCurve.rise().delay(delay), value: shown)
    }
}

extension View {
    func homeRise(_ shown: Bool, _ delay: Double) -> some View { modifier(HomeRise(shown: shown, delay: delay)) }
}

// MARK: - Bouton principal

/// Le filet clair le long du pan coupé (coin de signal du bouton principal).
struct HomeChamferEdge: Shape {
    var chamfer: CGFloat

    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.maxX + 1, y: r.maxY - chamfer - 1))
        p.addLine(to: CGPoint(x: r.maxX - chamfer - 1, y: r.maxY + 1))
        return p
    }
}

/// « Se connecter » : aplat Spectre, pan coupé bordé de givre. Hors ligne
/// (« Réessayer »), le même bouton en encre, sans lumière.
struct HomeCTAStyle: ButtonStyle {
    var online: Bool
    var height: CGFloat
    var fontSize: CGFloat
    /// Lumière du moment : l'allumage à l'arrivée, la veille au survol du pointeur.
    var glow: NeonLevel = .off

    /// Pan coupé à 8 pt du coin (mesuré sur la diagonale), filet de 1,8 pt.
    private static let cut: CGFloat = 8 * 2.squareRoot()
    private static let edge: CGFloat = 8.9 * 2.squareRoot()

    func makeBody(configuration: Configuration) -> some View {
        let shape = SignalCorner(radius: 9, chamfer: Self.cut)
        return configuration.label
            .font(Theme.sans(fontSize, .semibold))
            .foregroundStyle(Color.white)
            .frame(minHeight: height)
            .background(shape.fill(online ? Theme.accentFill : Theme.line))
            .overlay(
                HomeChamferEdge(chamfer: Self.edge)
                    .stroke(online ? Theme.neonCore : Theme.line2, lineWidth: 1.8)
                    .clipShape(shape)
            )
            .contentShape(shape)
            .neon(online ? (configuration.isPressed ? .flash : glow) : .off)
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
    }
}

/// Bouton rond de la barre du haut (QR code, ajout, réglages) : verre, 44 pt.
struct HomeCircleStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .foregroundStyle(Theme.text)
            .frame(width: 44, height: 44)
            .glass(22)
            .contentShape(Circle())
            .scaleEffect(configuration.isPressed ? 0.94 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
    }
}

/// Bouton en pilule de la barre du haut (iPad : « Scanner un QR code »).
struct HomePillStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Theme.sans(15, .medium))
            .foregroundStyle(Theme.text)
            .padding(.leading, 14).padding(.trailing, 18)
            .frame(height: 44)
            .glass(22)
            .contentShape(Capsule())
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
    }
}

/// Une carte touchée s'enfonce un peu (le retour du doigt).
struct HomePressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
    }
}

// MARK: - Profil

/// Jeu, Bureau, Max : un réglage d'un geste, sous le bouton principal. Le
/// profil retenu est souligné de Spectre ; « Personnalisé » n'en souligne aucun.
struct HomeProfilePicker: View {
    let selected: Profile?
    /// iPhone : trois parts égales sur toute la largeur ; iPad : à la taille du texte.
    var fill = true
    var height: CGFloat = 44
    var fontSize: CGFloat = 14
    let pick: (Profile) -> Void

    var body: some View {
        HStack(spacing: 3) {
            ForEach(Profile.allCases) { p in
                let on = selected == p
                Button { pick(p) } label: {
                    Text(p.label)
                        .font(Theme.sans(fontSize, .semibold))
                        .foregroundStyle(on ? Theme.text : Theme.muted)
                        .padding(.horizontal, fill ? 0 : 18)
                        .frame(maxWidth: fill ? .infinity : nil, minHeight: height)
                        .background {
                            if on {
                                RoundedRectangle(cornerRadius: 9, style: .continuous)
                                    .fill(Color.white.opacity(0.12))
                                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Color.white.opacity(0.1)))
                                    .overlay(alignment: .bottom) { Rectangle().fill(Theme.accent).frame(height: 2) }
                                    .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
                            }
                        }
                        .contentShape(Rectangle())
                }
                .buttonStyle(HomePressStyle())
                .accessibilityAddTraits(on ? [.isSelected] : [])
            }
        }
        .padding(4)
        .glass(12)
        .animation(.easeOut(duration: 0.2), value: selected)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Profil"))
    }
}

// MARK: - Point d'état

/// En ligne : point vert qui bat doucement ; hors ligne : anneau gris.
struct HomeStatusDot: View {
    var online: Bool
    var size: CGFloat = 7
    var pulse = true
    @State private var beat = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Group {
            if online {
                Circle().fill(Theme.ok)
                    .background(
                        Circle().fill(Theme.ok.opacity(beat ? 0 : 0.25))
                            .frame(width: size + (beat ? 12 : 6), height: size + (beat ? 12 : 6))
                    )
            } else {
                Circle().strokeBorder(Theme.faint, lineWidth: 1.5)
            }
        }
        .frame(width: size, height: size)
        .onAppear {
            guard pulse, !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 1.2).repeatForever(autoreverses: true)) { beat = true }
        }
        .accessibilityHidden(true)
    }
}

/// Pastille de verre (état, « Depuis votre Mac »).
struct HomeChip<Content: View>: View {
    var height: CGFloat = 30
    var fontSize: CGFloat = 13
    @ViewBuilder var content: Content

    var body: some View {
        HStack(spacing: 8) { content }
            .font(Theme.sans(fontSize))
            .foregroundStyle(Theme.text)
            .padding(.leading, 10).padding(.trailing, 12)
            .frame(height: height)
            .glass(height / 2)
    }
}

// MARK: - Cartes

/// Mesures des cartes : iPhone (rangée qui défile) ou iPad (six par ligne).
struct HomeCardMetrics {
    var width: CGFloat
    var height: CGFloat
    var image: CGFloat
    var radius: CGFloat
    var name: CGFloat
    var detail: CGFloat
    var padding: CGFloat

    static let phone = HomeCardMetrics(width: 150, height: 118, image: 64, radius: 14, name: 14, detail: 11.5, padding: 12)

    static func pad(width: CGFloat) -> HomeCardMetrics {
        HomeCardMetrics(width: width, height: 168, image: 94, radius: 16, name: 16, detail: 12.5, padding: 14)
    }
}

/// Un ordinateur de la rangée : sa dernière image (désaturée hors ligne),
/// son nom et son état ; celui qui est en scène est souligné de Spectre.
struct HomeHostCard: View {
    let host: Host
    let online: Bool
    let status: String
    let selected: Bool
    let m: HomeCardMetrics

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Color.clear
                .overlay { HostPicture(host: host, online: online) }
                .frame(width: m.width, height: m.image)
                .clipped()
            VStack(alignment: .leading, spacing: 4) {
                Text(host.name)
                    .font(Theme.sans(m.name, .semibold))
                    .foregroundStyle(online ? Theme.text : Theme.muted)
                    .lineLimit(1)
                HStack(spacing: 6) {
                    HomeStatusDot(online: online, size: 6, pulse: false)
                    Text(status).lineLimit(1)
                }
                .font(Theme.sans(m.detail))
                .foregroundStyle(Theme.muted)
            }
            .padding(.horizontal, m.padding)
            .padding(.vertical, m.padding - 3)
            Spacer(minLength: 0)
        }
        .frame(width: m.width, height: m.height, alignment: .topLeading)
        .glass(m.radius)
        .clipShape(RoundedRectangle(cornerRadius: m.radius, style: .continuous))
        .overlay(alignment: .bottom) {
            if selected {
                UnevenRoundedRectangle(topLeadingRadius: 2, topTrailingRadius: 2)
                    .fill(Theme.accent)
                    .frame(height: 3)
                    .padding(.horizontal, m.padding)
                    .shadow(color: Theme.accentFill, radius: 5)
                    .transition(.opacity)
            }
        }
        .contentShape(.contextMenuPreview, RoundedRectangle(cornerRadius: m.radius, style: .continuous))
        .contentShape(.hoverEffect, RoundedRectangle(cornerRadius: m.radius, style: .continuous))
    }
}

/// Un ordinateur annoncé sur le réseau et pas encore ajouté : il s'allume en
/// apparaissant (la veille du néon), puis respire.
struct HomeFoundCard: View {
    let name: String
    let detail: String?
    let m: HomeCardMetrics
    let wide: Bool
    @State private var arrived = false
    @State private var breathe = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: m.radius, style: .continuous)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 6) {
                Icon("wifi-high", size: wide ? 14 : 13, bold: true)
                Text("Sur le réseau".uppercased()).font(Theme.mono(wide ? 10 : 9.5, .medium)).tracking(0.8)
            }
            .foregroundStyle(Theme.accentLight)
            Spacer(minLength: 0)
            VStack(alignment: .leading, spacing: 3) {
                Text(name)
                    .font(Theme.display(wide ? 24 : 20, 640))
                    .tracking(-0.4)
                    .lineLimit(1)
                    .minimumScaleFactor(0.6)
                    .foregroundStyle(Theme.text)
                if let detail {
                    Text(detail).font(Theme.mono(wide ? 11 : 10)).foregroundStyle(Theme.muted).lineLimit(1)
                }
            }
            Spacer(minLength: 0)
            Text("Ajouter").font(Theme.sans(wide ? 14 : 13, .semibold)).foregroundStyle(Theme.accentLight)
        }
        .padding(wide ? 16 : 12)
        .frame(width: m.width, height: m.height, alignment: .leading)
        .background(shape.fill(.ultraThinMaterial).environment(\.colorScheme, .dark))
        .background(shape.fill(Theme.accentFill.opacity(0.14)))
        .clipShape(shape)
        .overlay(
            shape.strokeBorder(Theme.accent.opacity(arrived ? (breathe ? 0.45 : 0.75) : 1), lineWidth: 1)
                .shadow(color: Theme.accentFill.opacity(arrived ? (breathe ? 0.2 : 0.35) : 0.7), radius: arrived ? (breathe ? 10 : 18) : 26)
        )
        .contentShape(shape)
        .onAppear {
            guard !reduceMotion else { arrived = true; return }
            withAnimation(.easeOut(duration: 0.6)) { arrived = true }
            withAnimation(.easeInOut(duration: 1.6).delay(0.6).repeatForever(autoreverses: true)) { breathe = true }
        }
    }
}

/// « Ajouter un ordinateur » : rayures fines, encre, filet clair.
struct HomeAddCard: View {
    let m: HomeCardMetrics
    let wide: Bool

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: m.radius, style: .continuous)
        VStack(alignment: .leading, spacing: 0) {
            Icon("plus", size: wide ? 28 : 24).foregroundStyle(Theme.accent)
            Spacer(minLength: 0)
            VStack(alignment: .leading, spacing: 3) {
                Text("Ajouter un ordinateur").font(Theme.sans(m.name, .semibold)).foregroundStyle(Theme.text)
                Text(wide ? "iCloud, réseau, code, QR, adresse" : "iCloud, réseau, code, QR")
                    .font(Theme.sans(m.detail)).foregroundStyle(Theme.muted)
            }
        }
        .padding(wide ? 16 : 12)
        .frame(width: m.width, height: m.height, alignment: .leading)
        .background(HomeStripes(step: wide ? 13 : 10).opacity(0.03))
        .background(Theme.rgb(0x090C11, 0.5))
        .clipShape(shape)
        .overlay(shape.strokeBorder(Color.white.opacity(0.12)))
        .contentShape(shape)
    }
}

/// Rayures fines à 135° (carte d'ajout).
struct HomeStripes: View {
    var step: CGFloat = 10

    var body: some View {
        Canvas { ctx, size in
            var x: CGFloat = -size.height
            while x < size.width {
                var p = Path()
                p.move(to: CGPoint(x: x, y: size.height))
                p.addLine(to: CGPoint(x: x + size.height, y: 0))
                ctx.stroke(p, with: .color(.white), lineWidth: 1)
                x += step
            }
        }
        .accessibilityHidden(true)
    }
}

// MARK: - Ce que l'appareil voit

/// Clavier, trackpad ou souris branchés, et la sortie du son : la colonne
/// « Prêt à jouer » les suit quand on branche, débranche ou change de sortie.
@MainActor
final class HomePeripherals: ObservableObject {
    /// Nom du clavier matériel (« Magic Keyboard »), vide s'il n'en dit pas ; nil sans clavier.
    @Published private(set) var keyboard: String?
    @Published private(set) var mouse = false
    @Published private(set) var route = AudioRoute.current
    private var tokens: [NSObjectProtocol] = []

    init() {
        refresh()
        let names: [Notification.Name] = [.GCKeyboardDidConnect, .GCKeyboardDidDisconnect, .GCMouseDidConnect, .GCMouseDidDisconnect,
                                          AVAudioSession.routeChangeNotification]
        tokens = names.map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                Task { @MainActor in self?.refresh() }
            }
        }
    }

    deinit {
        tokens.forEach(NotificationCenter.default.removeObserver)
    }

    func refresh() {
        keyboard = GCKeyboard.coalesced.map { $0.vendorName ?? "" }
        mouse = !GCMouse.mice().isEmpty
        route = AudioRoute.current
    }
}

// MARK: - Prêt à jouer (iPad)

/// La colonne de l'Apple TV : manettes et batterie, sortie du son et son
/// retard, clavier et trackpad, puis la dernière session (latence médiane,
/// p95, et la règle des trames de l'écran).
struct HomeReadyCard: View {
    let host: Host?
    @ObservedObject var pads: GamepadBridge
    @ObservedObject var peripherals: HomePeripherals
    @ObservedObject private var prefs = Preferences.shared

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Prêt à jouer".uppercased()).font(Theme.mono(11, .medium)).tracking(0.9).foregroundStyle(Theme.muted)
            HomeReadyRows(host: host, pads: pads, peripherals: peripherals, size: 16)
            Rectangle().fill(Color.white.opacity(0.1)).frame(height: 1)
            lastSession
        }
        .padding(.horizontal, 24).padding(.vertical, 22)
        .glass(20)
    }

    @ViewBuilder private var lastSession: some View {
        HStack(alignment: .firstTextBaseline) {
            Text("Dernière session".uppercased()).font(Theme.mono(11, .medium)).tracking(0.9).foregroundStyle(Theme.muted)
            Spacer(minLength: 8)
            if let host, host.lastLatencyMs != nil {
                Text(sessionLine(host)).font(Theme.sans(12.5)).foregroundStyle(Theme.muted).lineLimit(1).minimumScaleFactor(0.8)
            }
        }
        if let host, let median = host.lastLatencyMs {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(verbatim: HomeView.ms(median)).font(Theme.mono(28, .medium)).foregroundStyle(Theme.text)
                Text(host.lastLatencyP95Ms.map { String(localized: "ms médiane · p95 \(HomeView.ms($0))") } ?? String(localized: "ms médiane"))
                    .font(Theme.mono(12.5)).foregroundStyle(Theme.muted)
            }
            HomeFrameRule(median: median, p95: host.lastLatencyP95Ms, refresh: DeviceDisplay.mode(for: host.settings).refresh)
                .frame(height: 30)
        } else {
            Text("Pas encore de session sur cet iPad.").font(Theme.sans(14)).foregroundStyle(Theme.muted)
        }
    }

    private func sessionLine(_ host: Host) -> String {
        let m = DeviceDisplay.mode(for: host.settings)
        var parts: [String] = []
        if let d = host.lastDuration { parts.append(HomeView.duration(d)) }
        parts.append("\(m.width) × \(m.height)")
        parts.append("\(m.refresh) Hz")
        parts.append(host.settings.codec == .h264 ? "H.264" : "HEVC")
        return parts.joined(separator: " · ")
    }
}

/// Les trois lignes de la colonne (manettes, son, clavier) ; aussi sous le
/// bouton en largeur réduite, quand la colonne disparaît.
struct HomeReadyRows: View {
    let host: Host?
    @ObservedObject var pads: GamepadBridge
    @ObservedObject var peripherals: HomePeripherals
    var size: CGFloat = 16
    @ObservedObject private var prefs = Preferences.shared

    var body: some View {
        VStack(alignment: .leading, spacing: size >= 16 ? 18 : 12) {
            row("game-controller", padsTitle, padsDetail, warn: false)
            let route = peripherals.route
            let late = route.isLate && prefs.latencyAlert
            row(route.kind == .airpods || route.kind == .headphones ? "headphones" : "speaker-high", soundTitle(route), soundDetail(route, late: late), warn: late)
            row(peripherals.keyboard != nil ? "keyboard" : "hand-pointing", inputTitle, inputDetail, warn: false)
        }
    }

    private func row(_ icon: String, _ title: String, _ detail: String, warn: Bool) -> some View {
        HStack(spacing: 14) {
            Icon(icon, size: size + 10).foregroundStyle(Theme.text)
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(Theme.sans(size, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                Text(detail).font(Theme.mono(size * 0.75)).foregroundStyle(warn ? Theme.warnText : Theme.muted).lineLimit(2)
            }
            Spacer(minLength: 0)
        }
        .accessibilityElement(children: .combine)
    }

    private var padsTitle: String {
        guard !pads.pads.isEmpty else { return String(localized: "Aucune manette") }
        return pads.pads.prefix(2).map { "\($0.label) \($0.name.replacingOccurrences(of: " Controller", with: ""))" }.joined(separator: " · ")
    }

    private var padsDetail: String {
        if pads.pads.isEmpty { return String(localized: "Allumez-en une en mode appairage") }
        let levels = pads.pads.prefix(2).compactMap(\.battery).map { String(localized: "\(Int(($0 * 100).rounded())) %") }
        var text = levels.isEmpty ? String(localized: "Prête") : String(localized: "Batterie \(levels.joined(separator: " · "))")
        if prefs.rumble { text += " · " + String(localized: "vibrations") }
        return text
    }

    private func soundTitle(_ route: AudioRoute) -> String {
        let name = route.name.prefix(1).uppercased() + route.name.dropFirst()
        if route.kind == .airpods, prefs.spatialAudio { return String(localized: "\(name) · audio spatial") }
        if route.channels > 2 { return "\(name) · \(route.channelsLabel)" }
        return name
    }

    private func soundDetail(_ route: AudioRoute, late: Bool) -> String {
        if host?.settings.audio == .off { return String(localized: "Le son reste sur l'ordinateur") }
        return late ? String(localized: "Sortie \(route.latencyMs) ms · le son arrivera après l'image")
                    : String(localized: "Sortie \(route.latencyMs) ms")
    }

    private var inputTitle: String {
        switch (peripherals.keyboard, peripherals.mouse) {
        case let (name?, true): return name.isEmpty ? String(localized: "Clavier et trackpad") : String(localized: "\(name) · trackpad")
        case let (name?, false): return name.isEmpty ? String(localized: "Clavier") : name
        case (nil, true): return String(localized: "Trackpad ou souris")
        case (nil, false): return prefs.directTouch ? String(localized: "Toucher direct") : String(localized: "Pavé tactile")
        }
    }

    private var inputDetail: String {
        if peripherals.keyboard != nil {
            return host?.settings.commandAsControl ?? true
                ? String(localized: "⌘ envoyée comme Ctrl · pointeur de l'ordinateur")
                : String(localized: "Pointeur de l'ordinateur")
        }
        if peripherals.mouse { return String(localized: "Pointeur de l'ordinateur") }
        return prefs.directTouch ? String(localized: "Le doigt est le pointeur") : String(localized: "Tout l'écran déplace le pointeur")
    }
}

/// Règle des trames : la latence médiane (trait Spectre) et sa dispersion
/// jusqu'au p95 (bande), face à la durée d'une image à la fréquence de la
/// session et à 60 Hz. Échelle : 0 à 20 ms.
struct HomeFrameRule: View {
    let median: Double
    let p95: Double?
    let refresh: Int

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width
            let x = { (ms: Double) in w * CGFloat(min(max(ms / 20, 0), 1)) }
            let high = p95 ?? median
            let low = max(0, median - (high - median))
            ZStack(alignment: .topLeading) {
                Rectangle().fill(Color.white.opacity(0.18)).frame(width: w, height: 2).offset(y: 16)
                RoundedRectangle(cornerRadius: 3)
                    .fill(Theme.accent.opacity(0.22))
                    .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(Theme.accent.opacity(0.5)))
                    .frame(width: max(4, x(high) - x(low)), height: 10)
                    .offset(x: x(low), y: 12)
                RoundedRectangle(cornerRadius: 2).fill(Theme.accent)
                    .frame(width: 3, height: 22)
                    .offset(x: x(median) - 1.5, y: 6)
                tick("\(refresh) Hz", at: x(1000 / Double(max(refresh, 1))), y: -4)
                if refresh != 60 { tick("60 Hz", at: x(1000.0 / 60), y: 22) }
            }
        }
        .accessibilityHidden(true)
    }

    private func tick(_ text: String, at x: CGFloat, y: CGFloat) -> some View {
        Text(verbatim: text)
            .font(Theme.mono(9))
            .foregroundStyle(Theme.faint)
            .fixedSize()
            .frame(width: 60)
            .offset(x: x - 30, y: y)
    }
}
