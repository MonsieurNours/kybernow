// Thème KyberNow pour l'iPhone et l'iPad, identité v2 (brand/v2, charte :
// brand/v2/charte/index.html) : encre, givre, signal Spectre en néon dans les
// moments actifs, Archivo (élargie pour les titres) et Martian Mono.
//
// Mêmes jetons que l'Apple TV (apps/tvos/KyberNowTV/Theme.swift) et le Mac,
// à l'échelle du pouce (cibles de 44 points au moins). Le toucher est
// l'éclair du néon ; le survol du pointeur de l'iPad, sa veille.
//
// Les jetons communs à l'iPhone, à l'iPad et à l'Apple TV (couleurs, polices,
// icônes, néon, symbole, étiquette du logo, coin de signal) sont dans
// ThemeTokens.swift ; ici, ce qui tient à l'échelle de cet appareil.

import CoreText
import KyberKit
import SwiftUI
import UIKit

extension Theme {
    static let bar = rgb(0x090D12)

    /// Étiquette en capitales (« VOS ORDINATEURS »).
    static func eyebrow(_ text: String, color: Color = muted) -> some View {
        Text(text.uppercased())
            .font(mono(11, .medium))
            .tracking(1.1)
            .foregroundStyle(color)
    }
}

/// La taille des icônes sur cet appareil (`Icon`, ThemeTokens.swift).
extension Icon {
    static let defaultSize: CGFloat = 20
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

// L'étiquette du logo signé : `SignatureTag` (ThemeTokens.swift).

// MARK: - Contrôles (toucher)
// Le coin de signal : `SignalCorner` (ThemeTokens.swift).
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
