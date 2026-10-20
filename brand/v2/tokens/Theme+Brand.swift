// Jetons KyberNow (identité v2, signal Spectre) pour SwiftUI. À fusionner dans
// apps/macos/KyberNow/Theme.swift.
// Polices : brand/v2/fonts (Archivo et Martian Mono variables, OFL), à copier dans le bundle.
// Archivo élargie : `.width(.expanded)` vise l'axe wdth (macOS 13+) ; à vérifier sur la police
// embarquée, sinon passer par un CTFontDescriptor avec la variation wdth = 125.

import SwiftUI

extension Theme {
    enum Brand {
        // Neutres
        static let ink0 = Theme.rgb(0x05070A)
        static let ink1 = Theme.rgb(0x090D12)
        static let ink2 = Theme.rgb(0x0E141A)
        static let ink3 = Theme.rgb(0x141B23)
        static let line = Theme.rgb(0x1E2832)
        static let line2 = Theme.rgb(0x2B3742)
        static let frost = Theme.rgb(0xE3EDF0)
        static let frost2 = Theme.rgb(0xB4C2C9)
        static let mist = Theme.rgb(0x8A99A3)
        static let faint = Theme.rgb(0x65737E)

        // Signal (Spectre)
        static let signal = Theme.rgb(0x9A8CFF)
        static let signalFill = Theme.rgb(0x6D5BFF)
        static let onSignal = Theme.rgb(0xFFFFFF)

        // États
        static let ok = Theme.rgb(0x4CD68A)
        static let warn = Theme.rgb(0xF5B841)
        static let bad = Theme.rgb(0xFF6B6B)

        // Formes
        static let radiusControl: CGFloat = 6
        static let radiusField: CGFloat = 10
        static let radiusCard: CGFloat = 16
        static let radiusSheet: CGFloat = 22
        static let chamfer: CGFloat = 10
        static let chamferCard: CGFloat = 22

        // Mouvement
        static let join = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.42)
        static let enter = Animation.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.32)
        static let hover = Animation.easeOut(duration: 0.2)

        static func display(_ size: CGFloat, _ weight: Font.Weight = .semibold) -> Font {
            .custom("Archivo", size: size).weight(weight).width(.expanded)
        }
        static func text(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
            .custom("Archivo", size: size).weight(weight)
        }
        static func mono(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
            .custom("Martian Mono", size: size).weight(weight).monospacedDigit()
        }
    }
}

/// Coin de signal : pan coupé à 45° en bas à droite.
struct SignalCorner: Shape {
    var radius: CGFloat = Theme.Brand.radiusControl
    var chamfer: CGFloat = Theme.Brand.chamfer

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

// MARK: - Lumière (néon)

/// Niveaux de la charte : 0 éteint (repos), 1 veille (survol, focus), 2 allumé (actif), 3 éclair (clic).
enum NeonLevel: Int {
    case off, hover, lit, flash
}

extension Theme.Brand {
    static let neonCore = Theme.rgb(0xF4F1FF)
}

/// Lueur à trois couches (cœur, gaine, halo). Coupée avec « Réduire la transparence ».
struct Neon: ViewModifier {
    var level: NeonLevel
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var breathing = false

    private var strength: (core: Double, sheath: Double, halo: Double, radius: CGFloat) {
        switch level {
        case .off: (0, 0, 0, 0)
        case .hover: (0, 0.8, 0.45, 8)
        case .lit: (1, 1, 0.8, 16)
        case .flash: (1, 1, 1, 28)
        }
    }

    func body(content: Content) -> some View {
        let s = strength
        let halo = s.halo * (level == .lit && breathing ? 0.7 : 1)
        return content
            .shadow(color: Theme.Brand.neonCore.opacity(reduceTransparency ? 0 : s.core), radius: 1)
            .shadow(color: Theme.Brand.signal.opacity(reduceTransparency ? 0 : s.sheath), radius: s.radius / 3)
            .shadow(color: Theme.Brand.signalFill.opacity(reduceTransparency ? 0 : halo), radius: s.radius)
            .animation(.easeOut(duration: level == .flash ? 0.04 : 0.2), value: level)
            .onChange(of: level, initial: true) { _, new in
                guard new == .lit, !reduceMotion else { breathing = false; return }
                withAnimation(.easeInOut(duration: 1.6).repeatForever(autoreverses: true)) { breathing = true }
            }
    }
}

/// Allumage : grésillement d'une seconde à l'apparition, une seule fois par déclencheur.
struct NeonIgnition<Trigger: Equatable>: ViewModifier {
    var trigger: Trigger
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    // Instants (s) et intensité, repris de la charte : 0, .06, .10, .16, .22, .30, .34, .44 → 1.
    private static var steps: [(Double, Double)] {
        [(0.06, 1), (0.04, 0.1), (0.06, 0.9), (0.06, 0.05), (0.08, 1), (0.04, 0.3), (0.10, 1), (0.56, 0)]
    }

    func body(content: Content) -> some View {
        if reduceMotion {
            content
        } else {
            content.keyframeAnimator(initialValue: 0.0, trigger: trigger) { view, glow in
                view
                    .shadow(color: Theme.Brand.neonCore.opacity(glow), radius: 1)
                    .shadow(color: Theme.Brand.signal.opacity(glow), radius: 5)
                    .shadow(color: Theme.Brand.signalFill.opacity(glow * 0.8), radius: 16)
            } keyframes: { _ in
                KeyframeTrack {
                    for (duration, value) in Self.steps {
                        LinearKeyframe(value, duration: duration)
                    }
                }
            }
        }
    }
}

extension View {
    func neon(_ level: NeonLevel) -> some View { modifier(Neon(level: level)) }
    func neonIgnition<T: Equatable>(trigger: T) -> some View { modifier(NeonIgnition(trigger: trigger)) }
}

/// Bouton principal : aplat Spectre, pan coupé, veille au survol, éclair au clic, allumé si `isLit`.
struct KNPrimaryButtonStyle: ButtonStyle {
    var isLit = false
    @State private var hovering = false

    func makeBody(configuration: Configuration) -> some View {
        let level: NeonLevel = configuration.isPressed ? .flash : isLit ? .lit : hovering ? .hover : .off
        return configuration.label
            .font(Theme.Brand.text(15, .semibold))
            .foregroundStyle(Theme.Brand.onSignal)
            .padding(.horizontal, 20)
            .frame(height: 44)
            .background(SignalCorner().fill(Theme.Brand.signalFill))
            .overlay(alignment: .bottomTrailing) {
                // Le pan coupé devient un liseré de néon dès que le bouton s'allume.
                Path { p in
                    p.move(to: CGPoint(x: Theme.Brand.chamfer, y: 0))
                    p.addLine(to: CGPoint(x: 0, y: Theme.Brand.chamfer))
                }
                .stroke(Theme.Brand.neonCore.opacity(level == .off ? 0 : 1), lineWidth: 1.5)
                .frame(width: Theme.Brand.chamfer, height: Theme.Brand.chamfer)
            }
            .neon(level)
            .onHover { hovering = $0 }
    }
}

extension ButtonStyle where Self == KNPrimaryButtonStyle {
    static var knPrimary: KNPrimaryButtonStyle { KNPrimaryButtonStyle() }
    static func knPrimary(isLit: Bool) -> KNPrimaryButtonStyle { KNPrimaryButtonStyle(isLit: isLit) }
}
