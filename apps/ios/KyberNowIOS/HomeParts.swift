// Pièces de l'accueil (maquettes iOS, planche 1) : le décor, le bouton
// principal et son pan coupé, le choix du profil, le point d'état, les
// cartes de la rangée « Vos ordinateurs ».

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
                                    .overlay(alignment: .bottom) {
                                        // Au-dessus du filet, pas sur lui.
                                        Capsule().fill(Theme.accent).frame(height: 2).padding(.horizontal, 14).padding(.bottom, 4)
                                    }
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

/// En ligne : point vert qui bat doucement ; hors ligne : anneau gris ;
/// « Recherche… » (nil) : point clair qui palpite.
struct HomeStatusDot: View {
    var online: Bool?
    var size: CGFloat = 7
    var pulse = true
    @State private var beat = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Group {
            switch online {
            case true?:
                Circle().fill(Theme.ok)
                    .background(
                        Circle().fill(Theme.ok.opacity(beat ? 0 : 0.25))
                            .frame(width: size + (beat ? 12 : 6), height: size + (beat ? 12 : 6))
                    )
            case nil:
                Circle().fill(Theme.text2).opacity(beat ? 0.35 : 1)
            case false?:
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

/// Pastille de verre (l'état de l'ordinateur).
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
    let online: Bool?
    let status: String
    let selected: Bool
    let m: HomeCardMetrics

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Color.clear
                .overlay { HostPicture(host: host, online: online != false) }
                .frame(width: m.width, height: m.image)
                .clipped()
            VStack(alignment: .leading, spacing: 4) {
                Text(host.name)
                    .font(Theme.sans(m.name, .semibold))
                    .foregroundStyle(online != false ? Theme.text : Theme.muted)
                    .lineLimit(1)
                    .minimumScaleFactor(0.75)
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
        .overlay(alignment: .bottom) {
            // À l'intérieur de la carte, au-dessus de son filet : posé sur le
            // bord, il le chevaucherait.
            if selected {
                Capsule()
                    .fill(Theme.accent)
                    .frame(height: 3)
                    .padding(.horizontal, m.padding)
                    .padding(.bottom, 6)
                    .shadow(color: Theme.accentFill, radius: 5)
                    .transition(.opacity)
            }
        }
        .glass(m.radius)
        .clipShape(RoundedRectangle(cornerRadius: m.radius, style: .continuous))
        .contentShape(.contextMenuPreview, RoundedRectangle(cornerRadius: m.radius, style: .continuous))
        .contentShape(.hoverEffect, RoundedRectangle(cornerRadius: m.radius, style: .continuous))
    }
}

/// Un ordinateur annoncé sur le réseau et pas encore ajouté : il s'allume en
/// apparaissant (la veille du néon), puis respire.
struct HomeFoundCard: View {
    let name: String
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
                Text(String(localized: "Sur le réseau").uppercased()).font(Theme.mono(wide ? 10 : 9.5, .medium)).tracking(0.8)
                    .lineLimit(2).fixedSize(horizontal: false, vertical: true)
            }
            .foregroundStyle(Theme.accentLight)
            Spacer(minLength: 0)
            Text(name)
                .font(Theme.display(wide ? 24 : 20, 640))
                .tracking(-0.4)
                .lineLimit(1)
                .minimumScaleFactor(0.5)
                .foregroundStyle(Theme.text)
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
            Text("Ajouter un ordinateur").font(Theme.sans(m.name, .semibold)).foregroundStyle(Theme.text)
                .fixedSize(horizontal: false, vertical: true)
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

/// « Rejoindre avec un code » (KyberNow Share) : la carte d'à côté, même facture.
struct HomeJoinCard: View {
    let m: HomeCardMetrics
    let wide: Bool

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: m.radius, style: .continuous)
        VStack(alignment: .leading, spacing: 0) {
            Icon("share-network", size: wide ? 28 : 24).foregroundStyle(Theme.accent)
            Spacer(minLength: 0)
            Text(ShareText.join).font(Theme.sans(m.name, .semibold)).foregroundStyle(Theme.text)
                .fixedSize(horizontal: false, vertical: true)
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
