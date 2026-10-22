// Thème KyberNow pour l'iPhone et l'iPad, identité v2 (brand/v2, charte :
// brand/v2/charte/index.html) : encre, givre, signal Spectre en néon dans les
// moments actifs, Archivo (élargie pour les titres) et Martian Mono.
//
// Mêmes jetons que l'Apple TV (apps/tvos/KyberNowTV/Theme.swift) et le Mac,
// à l'échelle du pouce (cibles de 44 points au moins). Le toucher est
// l'éclair du néon ; le survol du pointeur de l'iPad, sa veille.

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

    /// Marges latérales (iPhone) ; l'iPad en prend le double.
    static let gutter: CGFloat = 20
    static let top: CGFloat = 12

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
            .font(mono(11, .medium))
            .tracking(1.1)
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
    var size: CGFloat = 20
    var bold = false

    init(_ name: String, size: CGFloat = 20, bold: Bool = false) {
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
/// Le logo signé de KyberNow Link (charte : brand/v2/logo/kybernow-link-dark.svg,
/// géométrie : brand/v2/tools/geometry.json, `signed.link`) : sous le nom,
/// ferrée à droite sur le pied du « w », l'étiquette « LINK » au coin gauche
/// coupé à 45°. Sous un petit nom (barre du haut, en-tête), l'étiquette garde
/// une taille minimale (`minTagSize`) pour que « LINK » reste lisible.
/// L'étiquette pend sous le nom sans compter dans sa taille : à côté du
/// symbole, le nom reste centré sur lui, comme dans le logo signé.
struct Wordmark: View {
    var size: CGFloat = 28
    /// Signature « LINK » : affichée sauf `false`.
    var signed = true
    var signatureOpacity: Double = 1
    /// Taille de l'étiquette (en corps de nom) ; par défaut, `size`, et au
    /// moins `minTagSize`.
    var tagSize: CGFloat? = nil

    /// Plus petite étiquette : « LINK » en 7 points (0,26 corps).
    static let minTagSize: CGFloat = 27

    var body: some View {
        let name = (Text(verbatim: "Kyber").font(Font(Theme.variable("Archivo", size, weight: 420, width: 125)))
            + Text(verbatim: "Now").font(Font(Theme.variable("Archivo", size, weight: 720, width: 125))))
            .tracking(-size * 0.012)
            .foregroundStyle(Theme.text)
        Group {
            if signed {
                name.overlay(alignment: Alignment(horizontal: .trailing, vertical: .lastTextBaseline)) {
                    SignatureTag(size: tagSize ?? max(size, Self.minTagSize))
                        .opacity(signatureOpacity)
                        // Haut de l'étiquette à 0,13 corps sous la ligne de base…
                        .alignmentGuide(.lastTextBaseline) { d in d[.top] - max(size * 0.13, 2) }
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

// MARK: - Contrôles (toucher)

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

/// Bouton principal : aplat Spectre et pan coupé ; éclair du néon au toucher.
struct PrimaryButton: ButtonStyle {
    var height: CGFloat = 52
    var fontSize: CGFloat = 17

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Theme.sans(fontSize, .semibold))
            .foregroundStyle(Color.white)
            .frame(maxWidth: .infinity, minHeight: height)
            .background(SignalCorner(radius: 14, chamfer: 12).fill(Theme.accentFill))
            .overlay(SignalCorner(radius: 14, chamfer: 12).strokeBorder(Theme.accentLight.opacity(0.35)))
            .neon(configuration.isPressed ? .flash : .hover)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
            .contentShape(Rectangle())
    }
}

/// Bouton secondaire : verre sombre, filet clair.
struct GlassButton: ButtonStyle {
    var height: CGFloat = 44
    var fontSize: CGFloat = 15

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Theme.sans(fontSize, .medium))
            .foregroundStyle(Theme.text)
            .padding(.horizontal, 16)
            .frame(minHeight: height)
            .glass(12)
            .opacity(configuration.isPressed ? 0.7 : 1)
            .contentShape(Rectangle())
    }
}

/// Choix dans une liste (profil, réglage) : allumé quand il est retenu.
struct ChoiceButton: ButtonStyle {
    var selected: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Theme.sans(15, selected ? .semibold : .regular))
            .foregroundStyle(selected ? Theme.text : Theme.muted)
            .padding(.horizontal, 14)
            .frame(minHeight: 44)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(selected ? Theme.accentFill.opacity(0.28) : Theme.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .strokeBorder(selected ? Theme.accent.opacity(0.7) : Theme.line)
            )
            .opacity(configuration.isPressed ? 0.75 : 1)
            .contentShape(Rectangle())
    }
}

extension SignalCorner: InsettableShape {
    func inset(by amount: CGFloat) -> some InsettableShape {
        SignalCornerInset(base: self, amount: amount)
    }
}

struct SignalCornerInset: InsettableShape {
    var base: SignalCorner
    var amount: CGFloat

    func path(in rect: CGRect) -> Path {
        base.path(in: rect.insetBy(dx: amount, dy: amount))
    }

    func inset(by more: CGFloat) -> some InsettableShape {
        SignalCornerInset(base: base, amount: amount + more)
    }
}
