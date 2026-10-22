// Thème KyberNow pour l'Apple TV, identité v2 (brand/v2, charte :
// brand/v2/charte/index.html) : encre, givre, signal Spectre en néon dans les
// moments actifs, Archivo (élargie pour les titres) et Martian Mono.
//
// Mêmes jetons que l'app Mac (apps/macos/KyberNow/Theme.swift), à l'échelle
// d'un téléviseur vu à trois mètres (écran de 1920 × 1080 points). Le focus
// tvOS est la veille du néon : l'élément focalisé grossit, se soulève et
// s'entoure de givre et d'un halo Spectre.

import CoreText
import KyberKit
import SwiftUI
import UIKit

enum Theme {
    static func rgb(_ hex: UInt32, _ alpha: Double = 1) -> Color {
        Color(red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255, blue: Double(hex & 0xFF) / 255).opacity(alpha)
    }

    // Encre
    static let background = rgb(0x05070A)
    static let bar = rgb(0x090D12)
    static let surface = rgb(0x0E141A)
    static let surface2 = rgb(0x141B23)
    static let line = rgb(0x1E2832)
    static let line2 = rgb(0x2B3742)
    // Givre
    static let text = rgb(0xE3EDF0)
    static let text2 = rgb(0xC9D3DC)
    static let muted = rgb(0xB4C2C9)
    static let faint = rgb(0x8A99A3)
    static let ghost = rgb(0x65737E)
    // Signal (Spectre) et néon
    static let accent = rgb(0x9A8CFF)
    static let accentFill = rgb(0x6D5BFF)
    static let accentLight = rgb(0xD4CEFF)
    static let neonCore = rgb(0xF4F1FF)
    // États : jamais le signal.
    static let ok = rgb(0x4CD68A)
    static let warn = rgb(0xF5B841)
    static let warnText = rgb(0xEAD7A8)
    static let danger = rgb(0xFF6B6B)
    static let dangerText = rgb(0xFFC9C4)

    /// Marges sûres d'un téléviseur.
    static let gutter: CGFloat = 96
    static let top: CGFloat = 56

    // MARK: Polices

    static func registerFonts() {
        let urls = ["Archivo-wdth-wght", "MartianMono-wdth-wght"]
            .compactMap { Bundle.main.url(forResource: $0, withExtension: "ttf") }
        CTFontManagerRegisterFontURLs(urls as CFArray, .process, true, nil)
    }

    private static let wght = NSNumber(value: 0x7767_6874) // 'wght'
    private static let wdth = NSNumber(value: 0x7764_7468) // 'wdth'
    nonisolated(unsafe) private static var fonts: [String: UIFont] = [:]

    static func variable(_ family: String, _ size: CGFloat, weight: CGFloat, width: CGFloat) -> UIFont {
        let key = "\(family)|\(size)|\(weight)|\(width)"
        if let f = fonts[key] { return f }
        let desc = CTFontDescriptorCreateWithAttributes([
            kCTFontFamilyNameAttribute: family,
            kCTFontVariationAttribute: [wght: weight, wdth: width],
        ] as CFDictionary)
        let font = CTFontCreateWithFontDescriptor(desc, size, nil) as UIFont
        fonts[key] = font
        return font
    }

    private static func numeric(_ w: Font.Weight) -> CGFloat {
        switch w {
        case .light: return 320
        case .medium: return 500
        case .semibold: return 600
        case .bold: return 700
        case .heavy: return 780
        default: return 400
        }
    }

    /// Titres : Archivo élargie (largeur 125).
    static func display(_ size: CGFloat, _ weight: CGFloat = 660) -> Font {
        Font(variable("Archivo", size, weight: weight, width: 125))
    }

    static func sans(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        Font(variable("Archivo", size, weight: numeric(weight), width: 100))
    }

    static func mono(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        Font(variable("Martian Mono", size, weight: numeric(weight), width: 100))
    }

    /// Étiquette en capitales (« VOS ORDINATEURS »).
    static func eyebrow(_ text: String, color: Color = muted) -> some View {
        Text(text.uppercased())
            .font(mono(16, .medium))
            .tracking(1.3)
            .foregroundStyle(color)
    }

    static let easeOut = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.32)
    static let join = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.42)
    static let focus = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.28)
}

// MARK: - Icônes (Phosphor)

/// Icône Phosphor du catalogue (Assets.xcassets/Icons), en modèle.
struct Icon: View {
    let name: String
    var size: CGFloat = 28
    var bold = false

    init(_ name: String, size: CGFloat = 28, bold: Bool = false) {
        self.name = name
        self.size = size
        self.bold = bold
    }

    var body: some View {
        Image("Icons/\(name)\(bold ? "-bold" : "")")
            .renderingMode(.template)
            .resizable()
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

// MARK: - Lumière (néon)

/// Niveaux de la charte : éteint, veille (focus), allumé (actif), éclair (clic).
enum NeonLevel: Int, Comparable {
    case off, hover, lit, flash
    static func < (a: Self, b: Self) -> Bool { a.rawValue < b.rawValue }
}

struct NeonGlow: ViewModifier {
    var level: NeonLevel
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var breathe = false

    func body(content: Content) -> some View {
        let s: (core: Double, sheath: Double, halo: Double, radius: CGFloat) = switch level {
        case .off: (0, 0, 0, 0)
        case .hover: (0.3, 0.8, 0.5, 14)
        case .lit: (1, 1, breathe ? 0.5 : 0.85, 24)
        case .flash: (1, 1, 1, 40)
        }
        let k = reduceTransparency ? 0.0 : 1.0
        return content
            .shadow(color: Theme.neonCore.opacity(s.core * k), radius: 1.5)
            .shadow(color: Theme.accent.opacity(s.sheath * k), radius: max(1, s.radius / 3))
            .shadow(color: Theme.accentFill.opacity(s.halo * k), radius: max(1, s.radius))
            .animation(.easeOut(duration: level == .flash ? 0.04 : 0.2), value: level)
            .onChange(of: level, initial: true) { _, new in
                guard new == .lit, !reduceMotion else { breathe = false; return }
                withAnimation(.easeInOut(duration: 1.6).repeatForever(autoreverses: true)) { breathe = true }
            }
    }
}

extension View {
    func neon(_ level: NeonLevel) -> some View { modifier(NeonGlow(level: level)) }

    /// Verre sombre : flou de ce qui est derrière, voile d'encre, filet clair.
    func glass(_ radius: CGFloat = 24, tint: Double = 0.45) -> some View {
        background {
            RoundedRectangle(cornerRadius: radius, style: .continuous)
                .fill(.ultraThinMaterial)
                .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).fill(Theme.background.opacity(tint)))
                .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(Color.white.opacity(0.09)))
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

    static func source(in r: CGRect) -> Path { rounded([.zero, CGPoint(x: g - c, y: 0), CGPoint(x: 0, y: g - c)], [7, 1.2, 1.2], in: r) }
    static func dest(in r: CGRect) -> Path { rounded([CGPoint(x: g, y: c), CGPoint(x: g, y: g), CGPoint(x: c, y: g)], [1.2, 7, 1.2], in: r) }
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

/// Le symbole à l'état « now » (l'app), avec son néon.
struct Mark: View {
    var size: CGFloat = 34
    var level: NeonLevel = .off

    var body: some View {
        let s = size / 48
        let (halo, sheath, core): (Double, Double, Double) = switch level {
        case .off: (0, 0, 0)
        case .hover: (0.5, 0.7, 0.85)
        case .lit, .flash: (1, 1, 1)
        }
        ZStack {
            MarkPart(part: .source).fill(Theme.text)
            MarkPart(part: .dest).fill(level >= .lit ? Theme.rgb(0xAFA4FF) : Theme.accent)
            MarkPart(part: .seam)
                .stroke(Theme.accent, style: StrokeStyle(lineWidth: 9 * s, lineCap: .round))
                .blur(radius: 3.2 * s)
                .opacity(halo)
            MarkPart(part: .seam)
                .stroke(Theme.accent, style: StrokeStyle(lineWidth: 2.6 * s, lineCap: .round))
                .blur(radius: 0.7 * s)
                .opacity(sheath)
            MarkPart(part: .seam)
                .stroke(Theme.neonCore, style: StrokeStyle(lineWidth: 1.8 * s, lineCap: .round))
                .opacity(core)
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
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
    var size: CGFloat = 28
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
            .foregroundStyle(Theme.text)
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

// MARK: - Contrôles (focus tvOS)

/// Pan coupé à 45° en bas à droite : le coin de signal (un seul par écran).
struct SignalCorner: Shape {
    var radius: CGFloat = 12
    var chamfer: CGFloat = 13

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

private struct ChamferEdge: Shape {
    var chamfer: CGFloat
    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.maxX, y: r.maxY - chamfer))
        p.addLine(to: CGPoint(x: r.maxX - chamfer, y: r.maxY))
        return p
    }
}

/// Bouton principal : aplat Spectre et pan coupé ; allumé et respirant
/// quand il a le focus, éclair au clic.
struct PrimaryButton: ButtonStyle {
    var height: CGFloat = 80
    var fontSize: CGFloat = 28

    func makeBody(configuration: Configuration) -> some View {
        Styled(configuration: configuration, height: height, fontSize: fontSize)
    }

    private struct Styled: View {
        let configuration: Configuration
        let height: CGFloat
        let fontSize: CGFloat
        @Environment(\.isFocused) private var focused
        @Environment(\.isEnabled) private var enabled

        var body: some View {
            let chamfer = height * 0.15
            let level: NeonLevel = !enabled ? .off : configuration.isPressed ? .flash : focused ? .lit : .off
            configuration.label
                .font(Theme.sans(fontSize, .semibold))
                .padding(.horizontal, height * 0.45)
                .frame(height: height)
                .background(SignalCorner(radius: height * 0.15, chamfer: chamfer).fill(enabled ? Theme.accentFill : Theme.surface2))
                .overlay(ChamferEdge(chamfer: chamfer).stroke(Theme.neonCore, lineWidth: 2.4).opacity(level == .off ? 0 : 1))
                .foregroundStyle(enabled ? Color.white : Theme.faint)
                .neon(level)
                .scaleEffect(focused ? 1.06 : 1)
                .animation(Theme.focus, value: focused)
        }
    }
}

/// Élément focalisable en verre : carte, ligne de réglage, pastille.
/// Au focus : ×1,06, givre et halo Spectre, ombre portée.
struct GlassButton: ButtonStyle {
    var radius: CGFloat = 22
    var selected = false
    var scale: CGFloat = 1.06
    var fill: Color? = nil

    func makeBody(configuration: Configuration) -> some View {
        Styled(configuration: configuration, radius: radius, selected: selected, scale: scale, fill: fill)
    }

    private struct Styled: View {
        let configuration: Configuration
        let radius: CGFloat
        let selected: Bool
        let scale: CGFloat
        let fill: Color?
        @Environment(\.isFocused) private var focused

        var body: some View {
            let shape = RoundedRectangle(cornerRadius: radius, style: .continuous)
            configuration.label
                .foregroundStyle(Theme.text)
                .background {
                    if let fill {
                        shape.fill(fill)
                    } else {
                        shape.fill(.ultraThinMaterial)
                            .overlay(shape.fill(Theme.background.opacity(0.45)))
                            .overlay(shape.fill(Color.white.opacity(selected ? 0.1 : 0)))
                    }
                }
                .overlay(shape.strokeBorder(focused ? Theme.neonCore.opacity(0.92) : Color.white.opacity(selected ? 0.16 : 0.09), lineWidth: focused ? 2.5 : 1))
                .clipShape(shape)
                .shadow(color: Theme.accentFill.opacity(focused ? 0.55 : 0), radius: 26)
                .shadow(color: .black.opacity(focused ? 0.7 : 0), radius: 30, y: 24)
                .scaleEffect(configuration.isPressed ? scale * 0.97 : focused ? scale : 1)
                .animation(Theme.focus, value: focused)
                .animation(.easeOut(duration: 0.1), value: configuration.isPressed)
                .environment(\.colorScheme, .dark)
        }
    }
}

/// Pastille d'un choix (Jeu, Bureau, Max ; 60 Hz…) : soulignée de Spectre quand choisie.
struct ChoiceButton: ButtonStyle {
    var selected: Bool
    var height: CGFloat = 64
    var light = false

    func makeBody(configuration: Configuration) -> some View {
        Styled(configuration: configuration, selected: selected, height: height, light: light)
    }

    private struct Styled: View {
        let configuration: Configuration
        let selected: Bool
        let height: CGFloat
        let light: Bool
        @Environment(\.isFocused) private var focused
        @Environment(\.isEnabled) private var enabled

        var body: some View {
            let shape = RoundedRectangle(cornerRadius: 12, style: .continuous)
            configuration.label
                .font(Theme.sans(height * 0.34, .semibold))
                .padding(.horizontal, height * 0.38)
                .frame(height: height)
                .foregroundStyle(light && selected ? Theme.background : selected ? Theme.text : enabled ? Theme.text2 : Theme.ghost)
                .background(shape.fill(light && selected ? Theme.text : selected ? Color.white.opacity(0.12) : Color.white.opacity(light ? 0.08 : 0)))
                .overlay(alignment: .bottom) {
                    if selected, !light {
                        Rectangle().fill(Theme.accent).frame(height: 3).padding(.horizontal, 10)
                    }
                }
                .overlay(shape.strokeBorder(focused ? Theme.neonCore.opacity(0.92) : .clear, lineWidth: 2.5))
                .shadow(color: Theme.accentFill.opacity(focused ? 0.5 : 0), radius: 20)
                .scaleEffect(focused ? 1.08 : 1)
                .animation(Theme.focus, value: focused)
        }
    }
}

/// Interrupteur de la charte (Spectre allumé).
struct Toggle2: View {
    let on: Bool

    var body: some View {
        Capsule()
            .fill(on ? Theme.accentFill : Color.white.opacity(0.18))
            .frame(width: 64, height: 38)
            .overlay(alignment: on ? .trailing : .leading) {
                Circle().fill(.white).frame(width: 30, height: 30).padding(4)
            }
            .animation(Theme.easeOut, value: on)
            .accessibilityHidden(true)
    }
}

/// Glyphes d'aide en bas d'écran : ils suivent la dernière manette utilisée.
struct GlyphHints: View {
    @ObservedObject var pads = GamepadBridge.shared
    var choose = String(localized: "Choisir")
    var back = String(localized: "Retour")

    var body: some View {
        HStack(spacing: 30) {
            hint(glyph: .choose, choose)
            hint(glyph: .back, back)
        }
        .font(Theme.sans(19))
        .foregroundStyle(Theme.muted)
    }

    private enum Role { case choose, back }

    private func hint(glyph: Role, _ label: String) -> some View {
        HStack(spacing: 10) {
            Glyph(kind: PadGlyphKind(pads.lastKind), role: glyph == .choose ? .accept : .back)
            Text(label)
        }
    }
}

enum PadGlyphKind {
    case remote, xbox, playstation

    init(_ kind: GamepadBridge.Kind?) {
        switch kind {
        case .xbox: self = .xbox
        case .playstation: self = .playstation
        default: self = .remote
        }
    }
}

/// Un glyphe : pavé de la télécommande, A / B (Xbox), ✕ / ○ (PlayStation).
struct Glyph: View {
    enum Role { case accept, back }
    var kind: PadGlyphKind
    var role: Role
    var size: CGFloat = 30

    var body: some View {
        ZStack {
            switch (kind, role) {
            case (.remote, .accept):
                Circle().strokeBorder(Theme.muted, lineWidth: 1.6)
                Circle().fill(Theme.text).frame(width: size * 0.34)
            case (.remote, .back):
                Circle().strokeBorder(Theme.muted, lineWidth: 1.6)
                Image(systemName: "chevron.left").font(.system(size: size * 0.4, weight: .bold)).foregroundStyle(Theme.text)
            case (.xbox, .accept):
                Circle().fill(Theme.text)
                Text("A").font(Theme.sans(size * 0.5, .heavy)).foregroundStyle(Theme.background)
            case (.xbox, .back):
                Circle().strokeBorder(Theme.text, lineWidth: 1.8)
                Text("B").font(Theme.sans(size * 0.5, .heavy)).foregroundStyle(Theme.text)
            case (.playstation, .accept):
                Circle().fill(Theme.text)
                Image(systemName: "xmark").font(.system(size: size * 0.4, weight: .heavy)).foregroundStyle(Theme.background)
            case (.playstation, .back):
                Circle().strokeBorder(Theme.text, lineWidth: 1.8)
                Circle().strokeBorder(Theme.text, lineWidth: 2.2).frame(width: size * 0.38)
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}
