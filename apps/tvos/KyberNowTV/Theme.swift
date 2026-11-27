// Thème KyberNow pour l'Apple TV, identité v2 (brand/v2, charte :
// brand/v2/charte/index.html) : encre, givre, signal Spectre en néon dans les
// moments actifs, Archivo (élargie pour les titres) et Martian Mono.
//
// Mêmes jetons que l'app Mac (apps/macos/KyberNow/Theme.swift), à l'échelle
// d'un téléviseur vu à trois mètres (écran de 1920 × 1080 points). Le focus
// tvOS est la veille du néon : l'élément focalisé grossit, se soulève et
// s'entoure de givre et d'un halo Spectre.
//
// Les jetons communs à l'iPhone, à l'iPad et à l'Apple TV (couleurs, polices,
// icônes, néon, symbole, étiquette du logo, coin de signal) sont dans
// ThemeTokens.swift ; ici, ce qui tient à l'échelle de cet appareil.

import CoreText
import KyberKit
import SwiftUI
import UIKit

extension Theme {

    /// Étiquette en capitales (« VOS ORDINATEURS »).
    static func eyebrow(_ text: String, color: Color = muted) -> some View {
        Text(text.uppercased())
            .font(mono(16, .medium))
            .tracking(1.3)
            .foregroundStyle(color)
    }
}

/// La taille des icônes sur cet appareil (`Icon`, ThemeTokens.swift).
extension Icon {
    static let defaultSize: CGFloat = 28
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

// L'étiquette du logo signé : `SignatureTag` (ThemeTokens.swift).

// MARK: - Contrôles (focus tvOS)
// Le coin de signal : `SignalCorner` (ThemeTokens.swift).
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
