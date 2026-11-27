// Les jetons du thème KyberNow communs à l'iPhone, à l'iPad et à l'Apple TV
// (audit IOS-23 ; identité v2, charte : brand/v2/charte/index.html) :
// couleurs (encre, givre, signal Spectre), polices (Archivo, Martian Mono),
// animations, icônes Phosphor, néon, le symbole « Relais », l'étiquette du
// logo signé et le coin de signal. Chaque app garde dans son Theme.swift ce
// qui tient à son échelle (pouce ou téléviseur) : étiquettes, taille des
// icônes, nom, boutons. Compilé par les deux apps (apps/ios/project.yml le
// reprend, comme SessionModel.swift).

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
    /// Fond et bord d'un avertissement ambre (PyroWave en Wi-Fi, maquettes de PyroWave).
    static let warnFill = rgb(0x1E1A12)
    static let warnLine = rgb(0x4A3A1E)
    static let danger = rgb(0xFF6B6B)
    static let dangerText = rgb(0xFFC9C4)

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

    static let easeOut = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.32)
    static let join = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.42)
    static let focus = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.28)
}

// MARK: - Icônes (Phosphor)

/// Icône Phosphor du catalogue (Assets.xcassets/Icons), en modèle.
struct Icon: View {
    let name: String
    var size: CGFloat = Icon.defaultSize
    var bold = false

    init(_ name: String, size: CGFloat = Icon.defaultSize, bold: Bool = false) {
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

/// L'étiquette du logo signé : coin gauche coupé à 45°, coins droits à peine
/// arrondis, « LINK » en Martian Mono capitales (graisse 540). Proportions en
/// corps du nom, d'après brand/v2/tools/generate.mjs (`signedLogo`) : hauteur
/// 0,364, largeur 1,268, texte de 0,26 (approche 0,08 du sien), marge de 0,12
/// à droite et ligne de base à 0,08 du bas. Couleurs du thème sombre de la
/// charte : fond #1E2832, texte #C9D5DA.
struct SignatureTag: View {
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

struct SignatureTagShape: Shape {
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

