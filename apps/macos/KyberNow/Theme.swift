// Thème KyberNow, identité v2 (brand/v2, charte : brand/v2/charte/index.html).
// Encre presque noire et givre, un seul signal (Spectre) qui s'allume en néon
// dans les moments actifs, Archivo (élargie pour les titres) et Martian Mono
// pour les données. Icônes Phosphor (Assets.xcassets/Icons).

import AppKit
import CoreText
import SwiftUI

enum Theme {
    static func rgb(_ hex: UInt32, _ alpha: Double = 1) -> Color {
        Color(red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255, blue: Double(hex & 0xFF) / 255).opacity(alpha)
    }

    // Encre
    static let background = rgb(0x05070A)
    static let bar = rgb(0x090D12)
    static let rail = rgb(0x0C1218)
    static let surface = rgb(0x0E141A)
    static let surface2 = rgb(0x141B23)
    static let well = rgb(0x0A0F14)
    static let line = rgb(0x1E2832)
    static let line2 = rgb(0x2B3742)
    static let line3 = rgb(0x3A4855)
    // Givre
    static let text = rgb(0xE3EDF0)
    static let text2 = rgb(0xC9D3DC)
    static let muted = rgb(0xB4C2C9)
    static let faint = rgb(0x8A99A3)
    static let ghost = rgb(0x65737E)
    // Signal (Spectre) et néon
    static let accent = rgb(0x9A8CFF)
    static let accentFill = rgb(0x6D5BFF)
    static let accentInk = rgb(0xFFFFFF)
    static let accentLight = rgb(0xD4CEFF)
    static let accentSoft = rgb(0x6D5BFF, 0.14)
    static let accentLine = rgb(0x9A8CFF, 0.5)
    static let neonCore = rgb(0xF4F1FF)
    // États : jamais le signal.
    static let ok = rgb(0x4CD68A)
    static let warn = rgb(0xF5B841)
    static let warnText = rgb(0xEAD7A8)
    static let warnBg = rgb(0x1E1A12)
    static let warnLine = rgb(0x4A3A1E)
    static let danger = rgb(0xFF6B6B)
    static let dangerText = rgb(0xFFC9C4)
    static let dangerBg = rgb(0x2A1414)
    static let dangerLine = rgb(0x5C2A26)

    // MARK: Polices

    /// Polices livrées avec l'app (licence OFL), à enregistrer au lancement.
    static func registerFonts() {
        let urls = ["Archivo-wdth-wght", "MartianMono-wdth-wght"]
            .compactMap { Bundle.main.url(forResource: $0, withExtension: "ttf") }
        CTFontManagerRegisterFontURLs(urls as CFArray, .process, true, nil)
    }

    private static let wght = NSNumber(value: 0x7767_6874) // 'wght'
    private static let wdth = NSNumber(value: 0x7764_7468) // 'wdth'
    nonisolated(unsafe) private static var fonts: [String: NSFont] = [:]

    /// Instance d'une police variable, réglée par ses axes de graisse et de largeur.
    static func variable(_ family: String, _ size: CGFloat, weight: CGFloat, width: CGFloat) -> NSFont {
        let key = "\(family)|\(size)|\(weight)|\(width)"
        if let f = fonts[key] { return f }
        let desc = CTFontDescriptorCreateWithAttributes([
            kCTFontFamilyNameAttribute: family,
            kCTFontVariationAttribute: [wght: weight, wdth: width],
        ] as CFDictionary)
        let font = CTFontCreateWithFontDescriptor(desc, size, nil) as NSFont
        fonts[key] = font
        return font
    }

    private static func numeric(_ w: Font.Weight) -> CGFloat {
        switch w {
        case .ultraLight: return 200
        case .thin: return 250
        case .light: return 320
        case .medium: return 500
        case .semibold: return 600
        case .bold: return 700
        case .heavy: return 780
        case .black: return 860
        default: return 400
        }
    }

    /// Titres : Archivo élargie (largeur 125), approche serrée.
    static func display(_ size: CGFloat, _ weight: Font.Weight = .semibold) -> Font {
        let w: CGFloat = weight == .semibold ? 640 : weight == .bold ? 680 : numeric(weight)
        return Font(variable("Archivo", size, weight: w, width: 125))
    }

    /// Texte courant et interface : Archivo, largeur normale.
    static func sans(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        Font(variable("Archivo", size, weight: numeric(weight), width: 100))
    }

    /// Données (adresses, mesures, étiquettes) : Martian Mono, chiffres à chasse fixe.
    static func mono(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        Font(variable("Martian Mono", size, weight: numeric(weight), width: 100))
    }

    /// Étiquette en capitales (« PROFIL »).
    static func eyebrow(_ text: String, color: Color = faint) -> some View {
        Text(text.uppercased())
            .font(mono(10.5, .medium))
            .tracking(0.85)
            .foregroundStyle(color)
    }

    // MARK: Formes et mouvement

    static let radiusControl: CGFloat = 8
    static let radiusCard: CGFloat = 16
    static let radiusSheet: CGFloat = 22
    /// Sortie : cubic-bezier(.2, .9, .1, 1).
    static let easeOut = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.32)
    static let join = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.42)
}

// MARK: - Icônes (Phosphor)

/// Icône Phosphor du catalogue : regular au-dessus de 16 points, bold en dessous.
struct Icon: View {
    let name: String
    var size: CGFloat = 18
    var bold: Bool?

    init(_ name: String, size: CGFloat = 18, bold: Bool? = nil) {
        self.name = name
        self.size = size
        self.bold = bold
    }

    var body: some View {
        // Image de taille fixe (et non redimensionnable) : respectée aussi dans
        // les menus et barres d'outils, qui ignorent les cadres de SwiftUI.
        Image(nsImage: Self.image(name, size: size, bold: bold ?? (size <= 16)))
            .renderingMode(.template)
            .accessibilityHidden(true)
    }

    nonisolated(unsafe) private static var cache: [String: NSImage] = [:]

    static func image(_ name: String, size: CGFloat, bold: Bool) -> NSImage {
        let key = "\(name)|\(size)|\(bold)"
        if let image = cache[key] { return image }
        let image = (NSImage(named: "Icons/\(name)\(bold ? "-bold" : "")")?.copy() as? NSImage) ?? NSImage(size: NSSize(width: size, height: size))
        image.size = NSSize(width: size, height: size)
        image.isTemplate = true
        cache[key] = image
        return image
    }
}

/// Menu en verre (barre de session) : un bouton, et la liste dans une bulle.
struct GlassMenu<Label: View>: View {
    struct Item: Identifiable {
        let id: String
        let title: String
        var checked = false
        let action: () -> Void
    }

    let items: [Item]
    @ViewBuilder var label: Label
    @State private var open = false
    @State private var hovering = false

    var body: some View {
        Button { open.toggle() } label: {
            label
                .background(Color.white.opacity(hovering || open ? 0.07 : 0), in: RoundedRectangle(cornerRadius: 8))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .popover(isPresented: $open, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(items) { item in
                    MenuRow(item: item) { open = false }
                }
            }
            .padding(6)
            .frame(minWidth: 200)
            .preferredColorScheme(.dark)
        }
    }

    private struct MenuRow: View {
        let item: Item
        let close: () -> Void
        @State private var hovering = false

        var body: some View {
            Button {
                item.action()
                close()
            } label: {
                HStack(spacing: 10) {
                    Icon("check", size: 14, bold: true)
                        .foregroundStyle(Theme.accent)
                        .opacity(item.checked ? 1 : 0)
                    Text(item.title).font(Theme.sans(13.5, item.checked ? .semibold : .regular)).foregroundStyle(Theme.text)
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 10)
                .frame(height: 32)
                .background(hovering ? Theme.accentFill.opacity(0.35) : .clear, in: RoundedRectangle(cornerRadius: 6))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .onHover { hovering = $0 }
        }
    }
}

// MARK: - Lumière (néon)

/// Niveaux de la charte : 0 éteint (repos), 1 veille (survol, focus), 2 allumé (actif), 3 éclair (clic).
enum NeonLevel: Int, Comparable {
    case off, hover, lit, flash
    static func < (a: Self, b: Self) -> Bool { a.rawValue < b.rawValue }
}

/// Lueur à trois couches (cœur, gaine, halo) autour d'une forme. Coupée avec
/// « Réduire la transparence » ; la respiration, avec « Réduire les animations ».
struct NeonGlow: ViewModifier {
    var level: NeonLevel
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var breathe = false

    func body(content: Content) -> some View {
        let s: (core: Double, sheath: Double, halo: Double, radius: CGFloat) = switch level {
        case .off: (0, 0, 0, 0)
        case .hover: (0.2, 0.8, 0.45, 8)
        case .lit: (1, 1, breathe ? 0.55 : 0.8, 16)
        case .flash: (1, 1, 1, 28)
        }
        let k = reduceTransparency ? 0.0 : 1.0
        return content
            .shadow(color: Theme.neonCore.opacity(s.core * k), radius: 1)
            .shadow(color: Theme.accent.opacity(s.sheath * k), radius: max(1, s.radius / 3))
            .shadow(color: Theme.accentFill.opacity(s.halo * k), radius: max(1, s.radius))
            .animation(.easeOut(duration: level == .flash ? 0.04 : 0.2), value: level)
            .onChange(of: level, initial: true) { _, new in
                guard new == .lit, !reduceMotion else { breathe = false; return }
                withAnimation(.easeInOut(duration: 1.6).repeatForever(autoreverses: true)) { breathe = true }
            }
    }
}

/// Allumage : grésillement d'une seconde, une fois par valeur du déclencheur.
struct NeonIgnition<Trigger: Equatable>: ViewModifier {
    var trigger: Trigger
    var delay: Double = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// (durée, intensité) : 0, .06, .10, .16, .22, .30, .34, .44, puis retour.
    static var steps: [(Double, Double)] {
        [(0.06, 1), (0.04, 0.1), (0.06, 0.9), (0.06, 0.05), (0.08, 1), (0.04, 0.3), (0.10, 1), (0.56, 0)]
    }

    func body(content: Content) -> some View {
        if reduceMotion {
            content
        } else {
            content.keyframeAnimator(initialValue: 0.0, trigger: trigger) { view, glow in
                view
                    .shadow(color: Theme.neonCore.opacity(glow), radius: 1)
                    .shadow(color: Theme.accent.opacity(glow), radius: 5)
                    .shadow(color: Theme.accentFill.opacity(glow * 0.8), radius: 16)
            } keyframes: { _ in
                KeyframeTrack {
                    LinearKeyframe(0.0, duration: delay)
                    for (duration, value) in Self.steps {
                        LinearKeyframe(value, duration: duration)
                    }
                }
            }
        }
    }
}

extension View {
    func neon(_ level: NeonLevel) -> some View { modifier(NeonGlow(level: level)) }
    func neonIgnition<T: Equatable>(trigger: T, delay: Double = 0) -> some View { modifier(NeonIgnition(trigger: trigger, delay: delay)) }

    /// Verre sombre : flou de ce qui est derrière, voile d'encre, filet clair.
    func glass(_ radius: CGFloat = 12, tint: Double = 0.45) -> some View {
        background {
            RoundedRectangle(cornerRadius: radius, style: .continuous)
                .fill(.ultraThinMaterial)
                .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).fill(Theme.background.opacity(tint)))
                .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(Color.white.opacity(0.09)))
                .overlay(
                    RoundedRectangle(cornerRadius: radius, style: .continuous)
                        .strokeBorder(LinearGradient(colors: [Color.white.opacity(0.07), .clear], startPoint: .top, endPoint: .center))
                )
        }
        .environment(\.colorScheme, .dark)
    }
}

// MARK: - Le symbole « Relais »

/// Géométrie du symbole sur la grille de 48 (brand/v2/tools/generate.mjs).
enum MarkGeometry {
    private static let g: CGFloat = 48
    private static let c: CGFloat = 2 * 2.squareRoot()

    private static func rounded(_ pts: [CGPoint], _ radii: [CGFloat], in rect: CGRect) -> Path {
        let s = rect.width / g
        let p = pts.map { CGPoint(x: rect.minX + $0.x * s, y: rect.minY + $0.y * s) }
        var path = Path()
        let n = p.count
        path.move(to: CGPoint(x: (p[n - 1].x + p[0].x) / 2, y: (p[n - 1].y + p[0].y) / 2))
        for i in 0 ..< n {
            path.addArc(tangent1End: p[i], tangent2End: p[(i + 1) % n], radius: radii[i] * s)
        }
        path.closeSubpath()
        return path
    }

    /// L'ordinateur, en haut à gauche.
    static func source(in r: CGRect) -> Path { rounded([.zero, CGPoint(x: g - c, y: 0), CGPoint(x: 0, y: g - c)], [7, 1.2, 1.2], in: r) }
    /// Vous, en bas à droite.
    static func dest(in r: CGRect) -> Path { rounded([CGPoint(x: g, y: c), CGPoint(x: g, y: g), CGPoint(x: c, y: g)], [1.2, 7, 1.2], in: r) }
    /// Axe de la couture.
    static func seam(in r: CGRect) -> Path {
        let s = r.width / g
        var p = Path()
        p.move(to: CGPoint(x: r.minX + 5.6 * s, y: r.minY + 42.4 * s))
        p.addLine(to: CGPoint(x: r.minX + 42.4 * s, y: r.minY + 5.6 * s))
        return p
    }
}

struct MarkPart: Shape {
    enum Part { case source, dest, seam }
    let part: Part
    func path(in rect: CGRect) -> Path {
        switch part {
        case .source: MarkGeometry.source(in: rect)
        case .dest: MarkGeometry.dest(in: rect)
        case .seam: MarkGeometry.seam(in: rect)
        }
    }
}

/// Le symbole, dans l'un de ses trois états, avec son néon.
/// `now` : l'app et l'écosystème (la moitié « vous » allumée) ; `server` : la
/// moitié « ordinateur » ; `console` : la couture seule.
struct Mark: View {
    enum State { case now, server, console }

    var size: CGFloat = 44
    var state: State = .now
    var level: NeonLevel = .off
    /// Couleurs pour un fond clair ou une icône ; par défaut, givre et Spectre.
    var ice: Color = Theme.text
    var signal: Color = Theme.accent
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @SwiftUI.State private var breathe = false

    init(size: CGFloat = 44, state: State = .now, level: NeonLevel = .off, ice: Color = Theme.text, signal: Color = Theme.accent) {
        self.size = size
        self.state = state
        self.level = level
        self.ice = ice
        self.signal = signal
    }

    private var dim: Color { Theme.rgb(0x33404B) }

    var body: some View {
        let s = size / 48
        let (halo, sheath, core): (Double, Double, Double) = switch level {
        case .off: (0, 0, state == .console ? 1 : 0)
        case .hover: (0.5, 0.7, 0.85)
        case .lit: (breathe ? 0.7 : 1, 1, 1)
        case .flash: (1, 1, 1)
        }
        let litDest = state == .now && level >= .lit
        ZStack {
            MarkPart(part: .source).fill(state == .server ? signal : state == .console ? dim : ice)
            MarkPart(part: .dest).fill(state == .now ? (litDest ? Theme.accentLight.mix(signal, 0.6) : signal) : state == .console ? dim : ice)
            if !reduceTransparency {
                MarkPart(part: .seam)
                    .stroke(signal, style: StrokeStyle(lineWidth: (level == .flash ? 13 : 9) * s, lineCap: .round))
                    .blur(radius: 3.2 * s)
                    .opacity(halo)
                MarkPart(part: .seam)
                    .stroke(signal, style: StrokeStyle(lineWidth: 2.6 * s, lineCap: .round))
                    .blur(radius: 0.7 * s)
                    .opacity(sheath)
            }
            MarkPart(part: .seam)
                .stroke(level == .off && state == .console ? signal : Theme.neonCore, style: StrokeStyle(lineWidth: 1.8 * s, lineCap: .round))
                .opacity(core)
        }
        .frame(width: size, height: size)
        .animation(.easeOut(duration: 0.2), value: level)
        .onChange(of: level, initial: true) { _, new in
            guard new == .lit, !reduceMotion else { breathe = false; return }
            withAnimation(.easeInOut(duration: 1.6).repeatForever(autoreverses: true)) { breathe = true }
        }
        .accessibilityHidden(true)
    }
}

/// Allumage du symbole : la couture grésille une seconde, puis revient à son niveau.
struct MarkIgnition: View {
    var size: CGFloat
    var trigger: Int
    var delay: Double = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let s = size / 48
        if !reduceMotion {
            ZStack {
                MarkPart(part: .seam)
                    .stroke(Theme.accent, style: StrokeStyle(lineWidth: 9 * s, lineCap: .round))
                    .blur(radius: 3.2 * s)
                MarkPart(part: .seam)
                    .stroke(Theme.accent, style: StrokeStyle(lineWidth: 2.6 * s, lineCap: .round))
                    .blur(radius: 0.7 * s)
                MarkPart(part: .seam)
                    .stroke(Theme.neonCore, style: StrokeStyle(lineWidth: 1.8 * s, lineCap: .round))
            }
            .frame(width: size, height: size)
            .keyframeAnimator(initialValue: 0.0, trigger: trigger) { view, v in
                view.opacity(v)
            } keyframes: { _ in
                KeyframeTrack {
                    LinearKeyframe(0.0, duration: delay)
                    for (duration, value) in NeonIgnition<Int>.steps {
                        LinearKeyframe(value, duration: duration)
                    }
                }
            }
            .allowsHitTesting(false)
            .accessibilityHidden(true)
        }
    }
}

extension Color {
    /// Mélange simple de deux couleurs (sRGB).
    func mix(_ other: Color, _ t: Double) -> Color {
        let a = NSColor(self).usingColorSpace(.sRGB) ?? .white
        let b = NSColor(other).usingColorSpace(.sRGB) ?? .white
        return Color(red: a.redComponent + (b.redComponent - a.redComponent) * t,
                     green: a.greenComponent + (b.greenComponent - a.greenComponent) * t,
                     blue: a.blueComponent + (b.blueComponent - a.blueComponent) * t)
    }
}

/// Le nom : « Kyber » léger, « Now » gras, Archivo élargie.
///
/// À partir de `signedFrom` points, le logo signé de KyberNow Link (charte :
/// brand/v2/logo/kybernow-link-dark.svg, géométrie : brand/v2/tools/geometry.json,
/// `signed.link`) : sous le nom, ferrée à droite sur le pied du « w »,
/// l'étiquette « LINK » au coin gauche coupé à 45°. En dessous de cette
/// taille, le nom seul : la signature ne se lirait plus. L'étiquette pend
/// sous le nom sans compter dans sa taille : à côté du symbole, le nom reste
/// centré sur lui, comme dans le logo signé.
struct Wordmark: View {
    var size: CGFloat = 26
    var color: Color = Theme.text
    /// Signature « LINK » : `nil`, selon la taille ; `true` la force (vol du
    /// lancement sur iPhone et iPad, avec `signatureOpacity`).
    var signed: Bool? = nil
    var signatureOpacity: Double = 1

    /// Corps du nom à partir duquel la signature s'affiche : le logo signé
    /// fait alors 32 points de haut environ (1,35 corps).
    static let signedFrom: CGFloat = 24

    var body: some View {
        let name = (Text(verbatim: "Kyber").font(Font(Theme.variable("Archivo", size, weight: 420, width: 125)))
            + Text(verbatim: "Now").font(Font(Theme.variable("Archivo", size, weight: 720, width: 125))))
            .tracking(-size * 0.012)
            .foregroundStyle(color)
        Group {
            if signed ?? (size >= Self.signedFrom) {
                name.overlay(alignment: Alignment(horizontal: .trailing, vertical: .lastTextBaseline)) {
                    SignatureTag(size: size)
                        .opacity(signatureOpacity)
                        // Haut de l'étiquette à 0,13 corps sous la ligne de base…
                        .alignmentGuide(.lastTextBaseline) { d in d[.top] - size * 0.13 }
                        // … et son bord droit sur le pied du « w », à 0,22 corps de la fin du nom.
                        .alignmentGuide(.trailing) { d in d[.trailing] + size * 0.22 }
                }
            } else {
                name
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: "KyberNow Link"))
    }
}

/// L'étiquette du logo signé : coin gauche coupé à 45°, coins droits à peine
/// arrondis, « LINK » en Martian Mono capitales (graisse 540). Proportions en
/// corps du nom, d'après brand/v2/tools/generate.mjs (`signedLogo`) : hauteur
/// 0,364, largeur 1,268, texte de 0,26 (approche 0,08 du sien), marge de 0,12
/// à droite et ligne de base à 0,08 du bas. Couleurs du thème sombre de la
/// charte : fond #1E2832, texte #C9D5DA.
private struct SignatureTag: View {
    let size: CGFloat

    var body: some View {
        SignatureTagShape()
            .fill(Theme.rgb(0x1E2832))
            .frame(width: size * 1.268, height: size * 0.364)
            .alignmentGuide(.lastTextBaseline) { d in d[.bottom] - size * 0.08 }
            .overlay(alignment: Alignment(horizontal: .trailing, vertical: .lastTextBaseline)) {
                Text(verbatim: "LINK")
                    .font(Font(Theme.variable("Martian Mono", size * 0.26, weight: 540, width: 100)))
                    .tracking(size * 0.26 * 0.08)
                    .foregroundStyle(Theme.rgb(0xC9D5DA))
                    .fixedSize()
                    // 0,12 jusqu'à l'encre du « K », moins l'approche ajoutée après lui.
                    .padding(.trailing, size * 0.1)
            }
    }
}

private struct SignatureTagShape: Shape {
    func path(in r: CGRect) -> Path {
        let radius = r.height * 0.055
        var p = Path()
        p.move(to: CGPoint(x: r.minX + r.height, y: r.minY))
        p.addLine(to: CGPoint(x: r.maxX - radius, y: r.minY))
        p.addQuadCurve(to: CGPoint(x: r.maxX, y: r.minY + radius), control: CGPoint(x: r.maxX, y: r.minY))
        p.addLine(to: CGPoint(x: r.maxX, y: r.maxY - radius))
        p.addQuadCurve(to: CGPoint(x: r.maxX - radius, y: r.maxY), control: CGPoint(x: r.maxX, y: r.maxY))
        p.addLine(to: CGPoint(x: r.minX, y: r.maxY))
        p.closeSubpath()
        return p
    }
}

// MARK: - Contrôles

/// Pan coupé à 45° en bas à droite : le coin de signal (un seul par écran).
struct SignalCorner: Shape {
    var radius: CGFloat = 8
    var chamfer: CGFloat = 10

    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.minX + radius, y: r.minY))
        p.addLine(to: CGPoint(x: r.maxX - radius, y: r.minY))
        p.addQuadCurve(to: CGPoint(x: r.maxX, y: r.minY + radius), control: CGPoint(x: r.maxX, y: r.minY))
        p.addLine(to: CGPoint(x: r.maxX, y: r.maxY - chamfer))
        p.addLine(to: CGPoint(x: r.maxX - chamfer, y: r.maxY))
        p.addLine(to: CGPoint(x: r.minX + radius, y: r.maxY))
        p.addQuadCurve(to: CGPoint(x: r.minX, y: r.maxY - radius), control: CGPoint(x: r.minX, y: r.maxY))
        p.addLine(to: CGPoint(x: r.minX, y: r.minY + radius))
        p.addQuadCurve(to: CGPoint(x: r.minX + radius, y: r.minY), control: CGPoint(x: r.minX, y: r.minY))
        p.closeSubpath()
        return p
    }
}

/// Le liseré du pan coupé, qui devient un fil de néon quand le bouton s'allume.
private struct ChamferEdge: Shape {
    var chamfer: CGFloat
    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.maxX, y: r.maxY - chamfer))
        p.addLine(to: CGPoint(x: r.maxX - chamfer, y: r.maxY))
        return p
    }
}

/// Bouton principal : aplat Spectre, pan coupé ; veille au survol, éclair au
/// clic, allumé tant que `lit` (connexion en cours).
struct PrimaryButton: ButtonStyle {
    var height: CGFloat = 44
    var lit = false

    func makeBody(configuration: Configuration) -> some View {
        Styled(configuration: configuration, height: height, lit: lit)
    }

    private struct Styled: View {
        let configuration: Configuration
        let height: CGFloat
        let lit: Bool
        @Environment(\.isEnabled) private var enabled
        @State private var hovering = false

        var body: some View {
            let big = height > 46
            let chamfer: CGFloat = big ? 12 : 10
            let level: NeonLevel = !enabled ? .off : configuration.isPressed ? .flash : lit ? .lit : hovering ? .hover : .off
            configuration.label
                .font(Theme.sans(big ? 17 : 15, .semibold))
                .padding(.horizontal, big ? 24 : 20)
                .frame(height: height)
                .background(SignalCorner(radius: big ? 10 : 8, chamfer: chamfer).fill(enabled ? Theme.accentFill : Theme.surface2))
                .overlay(ChamferEdge(chamfer: chamfer).stroke(Theme.neonCore, lineWidth: 1.6).opacity(level == .off ? 0 : 1))
                .foregroundStyle(enabled ? Theme.accentInk : Theme.faint)
                .contentShape(Rectangle())
                .neon(level)
                .onHover { hovering = $0 }
                .animation(.easeOut(duration: 0.2), value: enabled)
        }
    }
}

/// Bouton secondaire : verre sombre et filet.
struct GhostButton: ButtonStyle {
    var height: CGFloat = 38
    var tint: Color = Theme.text
    var border: Color = Color.white.opacity(0.09)

    func makeBody(configuration: Configuration) -> some View {
        Styled(configuration: configuration, height: height, tint: tint, border: border)
    }

    private struct Styled: View {
        let configuration: Configuration
        let height: CGFloat
        let tint: Color
        let border: Color
        @State private var hovering = false

        var body: some View {
            configuration.label
                .font(Theme.sans(14))
                .padding(.horizontal, 14)
                .frame(height: height)
                .glass(9, tint: configuration.isPressed ? 0.15 : hovering ? 0.3 : 0.45)
                .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(border))
                .foregroundStyle(tint)
                .contentShape(Rectangle())
                .onHover { hovering = $0 }
        }
    }
}

/// Bouton d'action positive, pas principale (« Ajouter ») : voile de signal.
struct SignalButton: ButtonStyle {
    var height: CGFloat = 34

    func makeBody(configuration: Configuration) -> some View {
        Styled(configuration: configuration, height: height)
    }

    private struct Styled: View {
        let configuration: Configuration
        let height: CGFloat
        @State private var hovering = false

        var body: some View {
            configuration.label
                .font(Theme.sans(13.5, .semibold))
                .padding(.horizontal, 14)
                .frame(height: height)
                .background(Theme.accent.opacity(configuration.isPressed ? 0.3 : 0.18), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(Theme.accent.opacity(0.6)))
                .foregroundStyle(Theme.text)
                .neon(configuration.isPressed ? .flash : hovering ? .hover : .off)
                .contentShape(Rectangle())
                .onHover { hovering = $0 }
        }
    }
}

/// Sélecteur à segments : verre, choix souligné de Spectre, éclair au changement.
struct Segmented<Value: Hashable>: View {
    let options: [(Value, String)]
    @Binding var selection: Value?
    var disabled: Set<Value> = []
    var icons: [Value: String] = [:]
    var height: CGFloat = 34
    var fill = false
    @State private var flash: Value?

    var body: some View {
        HStack(spacing: 2) {
            ForEach(options, id: \.0) { value, label in
                let on = selection == value
                let off = disabled.contains(value)
                Button {
                    guard !off, selection != value else { return }
                    selection = value
                    flash = value
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { if flash == value { flash = nil } }
                } label: {
                    HStack(spacing: 8) {
                        if let icon = icons[value] {
                            Icon(icon, size: 17).foregroundStyle(on ? Theme.accent : off ? Theme.ghost : Theme.muted)
                        }
                        Text(label)
                    }
                    .font(Theme.sans(13.5, .semibold))
                    .padding(.horizontal, fill ? 0 : 15)
                    .frame(maxWidth: fill ? .infinity : nil)
                    .frame(height: height)
                    .background {
                        if on {
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(Color.white.opacity(0.08))
                                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(Color.white.opacity(0.1)))
                                .overlay(alignment: .bottom) {
                                    Rectangle().fill(Theme.accent).frame(height: 2).padding(.horizontal, 6)
                                }
                                .neon(flash == value ? .flash : .off)
                        }
                    }
                    .foregroundStyle(off ? Theme.ghost : on ? Theme.text : Theme.muted)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .disabled(off)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
        .padding(4)
        .glass(11)
        .animation(.easeOut(duration: 0.15), value: selection)
    }
}

/// Interrupteur : Spectre quand il est activé.
struct Switch: View {
    @Binding var isOn: Bool
    var label: String

    var body: some View {
        Button {
            isOn.toggle()
        } label: {
            ZStack(alignment: isOn ? .trailing : .leading) {
                Capsule().fill(isOn ? Theme.accentFill : Theme.surface2)
                    .overlay(Capsule().strokeBorder(isOn ? Color.clear : Theme.line2))
                Circle().fill(isOn ? Color.white : Theme.text2).padding(3)
            }
            .frame(width: 40, height: 24)
            .animation(.spring(response: 0.25, dampingFraction: 0.8), value: isOn)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .accessibilityValue(isOn ? Text("activé") : Text("désactivé"))
    }
}

/// Carte de réglages (icône, titre, contenu).
struct SettingsCard<Content: View>: View {
    let icon: String
    let title: String
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 10) {
                Icon(icon, size: 18)
                    .foregroundStyle(Theme.accent)
                    .frame(width: 32, height: 32)
                    .background(Theme.accentSoft, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                Text(title).font(Theme.display(17, .semibold)).foregroundStyle(Theme.text)
            }
            content
        }
        .padding(22)
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .glass(Theme.radiusCard, tint: 0.55)
    }
}

/// Bouton « i » qui ouvre une explication.
struct InfoTip<Content: View>: View {
    let label: String
    @ViewBuilder var content: Content
    @State private var open = false

    var body: some View {
        Button { open.toggle() } label: {
            Icon("info", size: 18)
                .foregroundStyle(Theme.faint)
                .frame(width: 24, height: 24)
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .popover(isPresented: $open, arrowEdge: .bottom) {
            VStack(alignment: .leading, spacing: 8) { content }
                .font(Theme.sans(13))
                .foregroundStyle(Theme.text2)
                .padding(16)
                .frame(width: 320, alignment: .leading)
        }
    }
}

/// Encadré d'information, d'avertissement ou d'alerte.
struct Notice: View {
    enum Kind { case info, warn, danger }
    let kind: Kind
    let text: String

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Icon(kind == .info ? "info" : "warning", size: 17)
                .foregroundStyle(kind == .info ? Theme.faint : kind == .warn ? Theme.warn : Theme.danger)
            Text(text)
                .font(Theme.sans(12.5))
                .foregroundStyle(kind == .info ? Theme.muted : kind == .warn ? Theme.warnText : Theme.dangerText)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 14).padding(.vertical, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(kind == .info ? Color.white.opacity(0.03) : kind == .warn ? Theme.warnBg : Theme.dangerBg, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(kind == .info ? Color.white.opacity(0.08) : kind == .warn ? Theme.warnLine : Theme.dangerLine))
    }
}

/// Pastille d'état d'un ordinateur. Le point porte l'état, le texte le précise ;
/// le signal n'y apparaît jamais.
struct StatusPill: View {
    let online: Bool?
    let text: String
    @State private var pulse = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 9) {
            ZStack {
                if online == true {
                    Circle().fill(Theme.ok)
                    Circle().stroke(Theme.ok.opacity(pulse ? 0 : 0.5), lineWidth: pulse ? 6 : 1)
                } else if online == nil {
                    Circle().fill(Theme.text2).opacity(pulse ? 0.35 : 1)
                } else {
                    Circle().strokeBorder(Theme.faint, lineWidth: 1.2)
                }
            }
            .frame(width: 7, height: 7)
            Text(text).font(Theme.sans(13))
        }
        .padding(.leading, 10).padding(.trailing, 12)
        .frame(height: 30)
        .glass(15)
        .foregroundStyle(online == false ? Theme.faint : Theme.text)
        .onAppear {
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: online == nil ? 1.2 : 2.4).repeatForever(autoreverses: online == nil)) { pulse = true }
        }
    }
}

/// Règle de trame : la latence lue contre la durée d'une image (160, 120, 60 Hz).
struct FrameRuler: View {
    var median: Double
    var p95: Double?
    var live = false
    /// Échelle en millisecondes : 20, ou plus si la mesure la dépasse.
    var scale: Double? = nil

    var body: some View {
        let scale = self.scale ?? max(20, ((max(median, p95 ?? 0) * 1.25) / 10).rounded(.up) * 10)
        GeometryReader { geo in
            let w = geo.size.width
            let x = { (ms: Double) in CGFloat(min(ms, scale) / scale) * w }
            ZStack(alignment: .topLeading) {
                Rectangle().fill(Color.white.opacity(0.16)).frame(height: 2).offset(y: 16)
                if let p95 {
                    let lo = max(0, median - (p95 - median) * 0.6)
                    RoundedRectangle(cornerRadius: 3)
                        .fill(Theme.accent.opacity(live ? 0.26 : 0.2))
                        .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(Theme.accent.opacity(0.5)))
                        .frame(width: max(4, x(p95) - x(lo)), height: 10)
                        .offset(x: x(lo), y: 12)
                }
                // Repères d'une image à 160, 120 et 60 Hz ; une étiquette seulement si elle a la place.
                let ticks = [(1000 / 160.0, "160 Hz", true), (1000 / 120.0, "120 Hz", false), (1000 / 60.0, "60 Hz", false)]
                ForEach(ticks, id: \.1) { ms, label, up in
                    let crowded = ticks.contains { $0.2 == up && $0.0 > ms && x($0.0) - x(ms) < 38 }
                    Rectangle().fill(Theme.faint.opacity(0.7)).frame(width: 1, height: 18).offset(x: x(ms), y: 8)
                    if !crowded {
                        Text(label).font(Theme.mono(9)).foregroundStyle(Theme.faint)
                            .fixedSize()
                            .offset(x: x(ms) - 18, y: up ? -6 : 28)
                    }
                }
                RoundedRectangle(cornerRadius: 2)
                    .fill(live ? Theme.neonCore : Theme.accent)
                    .frame(width: 3, height: 24)
                    .offset(x: x(median) - 1.5, y: 5)
                    .neon(live ? .lit : .off)
                    .animation(Theme.easeOut, value: median)
            }
        }
        .frame(height: 40)
        .accessibilityElement()
        .accessibilityLabel(Text("Latence médiane \(median, specifier: "%.1f") millisecondes"))
    }
}
