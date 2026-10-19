// Thème KyberNow (maquette Claude Design « KyberNow macOS ») : sombre,
// menthe #6FE3C1, Bricolage Grotesque pour les titres, IBM Plex pour le reste.

import AppKit
import CoreText
import SwiftUI

enum Theme {
    static func rgb(_ hex: UInt32, _ alpha: Double = 1) -> Color {
        Color(red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255, blue: Double(hex & 0xFF) / 255).opacity(alpha)
    }

    static let background = rgb(0x0D1117)
    static let bar = rgb(0x0A0E13)
    static let rail = rgb(0x10151C)
    static let surface = rgb(0x121922)
    static let surface2 = rgb(0x18202B)
    static let well = rgb(0x0F151D)
    static let line = rgb(0x1F2733)
    static let line2 = rgb(0x2B3646)
    static let line3 = rgb(0x3A4A5E)
    static let text = rgb(0xE9EEF5)
    static let text2 = rgb(0xC9D3DF)
    static let muted = rgb(0x9AA7B8)
    static let faint = rgb(0x8794A6)
    static let accent = rgb(0x6FE3C1)
    static let accentLight = rgb(0x9FF0D8)
    static let accentInk = rgb(0x06251D)
    static let accentSoft = rgb(0x1C3A33)
    static let accentLine = rgb(0x3D7A69)
    static let warn = rgb(0xF5B65A)
    static let warnText = rgb(0xD9C59E)
    static let warnBg = rgb(0x1E1A12)
    static let warnLine = rgb(0x4A3A1E)
    static let danger = rgb(0xFF7B72)
    static let dangerText = rgb(0xFFC9C4)
    static let dangerBg = rgb(0x2A1414)
    static let dangerLine = rgb(0x5C2A26)

    /// Polices livrées avec l'app (licence OFL), à enregistrer au lancement.
    static func registerFonts() {
        let urls = ["BricolageGrotesque-opsz-wdth-wght", "IBMPlexSans-wdth-wght", "IBMPlexMono-Regular", "IBMPlexMono-Medium"]
            .compactMap { Bundle.main.url(forResource: $0, withExtension: "ttf") }
        CTFontManagerRegisterFontURLs(urls as CFArray, .process, true, nil)
    }

    static func display(_ size: CGFloat, _ weight: Font.Weight = .semibold) -> Font {
        .custom("Bricolage Grotesque", size: size).weight(weight)
    }

    static func sans(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        .custom("IBM Plex Sans", size: size).weight(weight)
    }

    static func mono(_ size: CGFloat, _ weight: Font.Weight = .regular) -> Font {
        .custom(weight == .medium ? "IBMPlexMono-Medium" : "IBMPlexMono-Regular", size: size)
    }

    /// Petit intertitre en capitales (« PROFIL »).
    static func eyebrow(_ text: String) -> some View {
        Text(text.uppercased())
            .font(sans(12, .semibold))
            .tracking(1)
            .foregroundStyle(faint)
    }
}

// MARK: - Logo

/// Le « K » de KyberNow et ses ondes (même dessin que brand/kybernow-mark.svg).
struct Mark: View {
    var size: CGFloat = 44
    var animated = true

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30, paused: !animated)) { timeline in
            canvas(phase: animated ? CGFloat(timeline.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 2.4) / 2.4 * 200) : 0)
        }
        .frame(width: size, height: size)
    }

    private func canvas(phase: CGFloat) -> some View {
        Canvas { ctx, canvas in
            let s = canvas.width / 512
            func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: x * s, y: y * s) }
            for (dy, alpha) in [(-14.0, 0.5), (0.0, 1.0), (14.0, 0.5)] {
                var wave = Path()
                wave.move(to: p(70, 262 + dy))
                wave.addLine(to: p(252, 262 + dy))
                wave.addCurve(to: p(300, 312 + dy), control1: p(272, 262 + dy), control2: p(280, 312 + dy))
                wave.addCurve(to: p(346, 156 + dy), control1: p(322, 312 + dy), control2: p(326, 156 + dy))
                wave.addCurve(to: p(388, 262 + dy), control1: p(366, 156 + dy), control2: p(368, 262 + dy))
                wave.addLine(to: p(470, 262 + dy))
                let style = animated
                    ? StrokeStyle(lineWidth: 9 * s, lineCap: .round, lineJoin: .round, dash: [140 * s, 60 * s], dashPhase: -phase * s)
                    : StrokeStyle(lineWidth: 9 * s, lineCap: .round, lineJoin: .round)
                ctx.stroke(wave, with: .color(Theme.accent.opacity(alpha)), style: style)
            }
            var arm = Path()
            arm.addLines([p(238, 264.5), p(404, 416), p(330, 416), p(206, 306)])
            ctx.fill(arm, with: .color(Theme.rgb(0xC9D2DC)))
            var top = Path()
            top.addLines([p(206, 224), p(321, 96), p(392, 96), p(206, 302)])
            ctx.fill(top, with: .color(Theme.rgb(0xF4F7FA)))
            ctx.fill(Path(roundedRect: CGRect(x: 150 * s, y: 96 * s, width: 56 * s, height: 320 * s), cornerRadius: 38 * s), with: .color(Theme.rgb(0xF4F7FA)))
        }
    }
}

struct Wordmark: View {
    var size: CGFloat = 26

    var body: some View {
        (Text("Kyber").foregroundColor(Theme.rgb(0xF4F7FA)) + Text("Now").foregroundColor(Theme.accent))
            .font(Theme.display(size, .bold))
    }
}

// MARK: - Fond « signal »

/// Ondes qui traversent le bas de la fenêtre (comme l'installateur).
struct SignalBackground: View {

    private func wave(_ w: CGFloat, _ h: CGFloat, base: CGFloat, dip: CGFloat, peak: CGFloat, x0: CGFloat) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: -20, y: base))
        p.addLine(to: CGPoint(x: x0, y: base))
        p.addCurve(to: CGPoint(x: x0 + 120, y: dip), control1: CGPoint(x: x0 + 50, y: base), control2: CGPoint(x: x0 + 70, y: dip))
        p.addCurve(to: CGPoint(x: x0 + 240, y: peak), control1: CGPoint(x: x0 + 180, y: dip), control2: CGPoint(x: x0 + 190, y: peak))
        p.addCurve(to: CGPoint(x: x0 + 350, y: base), control1: CGPoint(x: x0 + 290, y: peak), control2: CGPoint(x: x0 + 300, y: base))
        p.addLine(to: CGPoint(x: w + 20, y: base))
        return p
    }

    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30)) { timeline in
            let flow = CGFloat(timeline.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 9) / 9)
            Canvas { ctx, size in
                let w = size.width, h = size.height
                let a = wave(w, h, base: h * 0.76, dip: h * 0.85, peak: h * 0.59, x0: w * 0.33)
                let b = wave(w, h, base: h * 0.81, dip: h * 0.88, peak: h * 0.70, x0: w * 0.41)
                let dash = [w * 0.18, w * 1.2]
                ctx.stroke(a, with: .color(Theme.accent.opacity(0.07)), lineWidth: 2)
                ctx.stroke(a, with: .color(Theme.accent.opacity(0.35)), style: StrokeStyle(lineWidth: 2, lineCap: .round, dash: dash, dashPhase: -flow * w * 1.38))
                ctx.stroke(b, with: .color(Theme.accent.opacity(0.05)), lineWidth: 2)
                ctx.stroke(b, with: .color(Theme.accent.opacity(0.25)), style: StrokeStyle(lineWidth: 2, lineCap: .round, dash: dash, dashPhase: -(flow + 0.43) * w * 1.38))
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

// MARK: - Contrôles

struct PrimaryButton: ButtonStyle {
    var height: CGFloat = 44

    func makeBody(configuration: Configuration) -> some View {
        Label(configuration: configuration, height: height)
    }

    private struct Label: View {
        let configuration: Configuration
        let height: CGFloat
        @Environment(\.isEnabled) private var enabled

        var body: some View {
            configuration.label
                .font(Theme.sans(height > 46 ? 16 : 15, .semibold))
                .padding(.horizontal, height > 46 ? 26 : 22)
                .frame(height: height)
                .background(enabled ? (configuration.isPressed ? Theme.rgb(0x8AEBCF) : Theme.accent) : Theme.surface2, in: RoundedRectangle(cornerRadius: height > 46 ? 12 : 11))
                .foregroundStyle(enabled ? Theme.accentInk : Theme.muted)
                .contentShape(Rectangle())
                .animation(.easeOut(duration: 0.2), value: enabled)
        }
    }
}

struct GhostButton: ButtonStyle {
    var height: CGFloat = 38
    var tint: Color = Theme.text
    var border: Color = Theme.line2

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Theme.sans(14))
            .padding(.horizontal, 14)
            .frame(height: height)
            .background(configuration.isPressed ? Theme.surface2 : .clear, in: RoundedRectangle(cornerRadius: 9))
            .overlay(RoundedRectangle(cornerRadius: 9).stroke(border))
            .foregroundStyle(tint)
            .contentShape(Rectangle())
    }
}

/// Sélecteur à segments de la maquette (fond creux, choix en menthe).
struct Segmented<Value: Hashable>: View {
    let options: [(Value, String)]
    @Binding var selection: Value?
    var disabled: Set<Value> = []
    var height: CGFloat = 34
    var fill = false

    var body: some View {
        HStack(spacing: 4) {
            ForEach(options, id: \.0) { value, label in
                let on = selection == value
                let off = disabled.contains(value)
                Button {
                    if !off { selection = value }
                } label: {
                    Text(label)
                        .font(Theme.sans(13, .semibold))
                        .padding(.horizontal, fill ? 0 : 16)
                        .frame(maxWidth: fill ? .infinity : nil)
                        .frame(height: height)
                        .background(on ? Theme.accent : .clear, in: RoundedRectangle(cornerRadius: 8))
                        .foregroundStyle(off ? Theme.rgb(0x5B6778) : on ? Theme.accentInk : Theme.text2)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .disabled(off)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
        .padding(4)
        .background(Theme.bar, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Theme.line))
        .animation(.easeOut(duration: 0.15), value: selection)
    }
}

/// Interrupteur menthe de la maquette.
struct Switch: View {
    @Binding var isOn: Bool
    var label: String

    var body: some View {
        Button {
            isOn.toggle()
        } label: {
            ZStack(alignment: isOn ? .trailing : .leading) {
                Capsule().fill(isOn ? Theme.accent : Theme.line2)
                Circle().fill(isOn ? Theme.accentInk : Theme.text2).padding(3)
            }
            .frame(width: 44, height: 26)
            .animation(.spring(response: 0.25, dampingFraction: 0.8), value: isOn)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .accessibilityValue(isOn ? "activé" : "désactivé")
    }
}

/// Carte de réglages (icône, titre, contenu).
struct SettingsCard<Content: View>: View {
    let symbol: String
    let title: String
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 10) {
                Image(systemName: symbol)
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(Theme.accent)
                    .frame(width: 32, height: 32)
                    .background(Theme.accentSoft, in: RoundedRectangle(cornerRadius: 9))
                Text(title).font(Theme.sans(17, .semibold)).foregroundStyle(Theme.text)
            }
            content
        }
        .padding(22)
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .background(Theme.surface.opacity(0.92), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(Theme.line2))
    }
}

/// Bouton « i » qui ouvre une explication.
struct InfoTip<Content: View>: View {
    let label: String
    @ViewBuilder var content: Content
    @State private var open = false

    var body: some View {
        Button { open.toggle() } label: {
            Text("i")
                .font(Theme.sans(12, .semibold))
                .foregroundStyle(Theme.muted)
                .frame(width: 22, height: 22)
                .overlay(Circle().stroke(Theme.line3))
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
            Image(systemName: kind == .info ? "info.circle" : kind == .warn ? "exclamationmark.triangle" : "exclamationmark.octagon")
                .font(.system(size: 14))
                .foregroundStyle(kind == .info ? Theme.accent : kind == .warn ? Theme.warn : Theme.danger)
                .padding(.top, 1)
            Text(text)
                .font(Theme.sans(12))
                .foregroundStyle(kind == .info ? Theme.muted : kind == .warn ? Theme.warnText : Theme.dangerText)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 14).padding(.vertical, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(kind == .info ? Theme.well : kind == .warn ? Theme.warnBg : Theme.dangerBg, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(kind == .info ? Theme.line : kind == .warn ? Theme.warnLine : Theme.dangerLine))
    }
}

/// Pastille d'état (« En ligne · réseau local »).
struct StatusPill: View {
    let online: Bool?
    let text: String
    @State private var pulse = false

    var body: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(online == true ? Theme.accent : Theme.rgb(0x5B6778))
                .frame(width: 8, height: 8)
                .overlay(
                    Circle().stroke(Theme.accent.opacity(pulse ? 0 : 0.55), lineWidth: pulse ? 6 : 0)
                        .opacity(online == true ? 1 : 0)
                )
            Text(text).font(Theme.sans(13, .medium))
        }
        .padding(.horizontal, 12)
        .frame(height: 28)
        .background(online == true ? Theme.accentSoft : Theme.surface2, in: Capsule())
        .foregroundStyle(online == true ? Theme.accentLight : Theme.muted)
        .onAppear {
            withAnimation(.easeInOut(duration: 2.2).repeatForever(autoreverses: false)) { pulse = true }
        }
    }
}

/// Moniteur stylisé de la carte d'accueil, avec son onde animée.
struct MonitorArt: View {
    var body: some View {
        TimelineView(.animation(minimumInterval: 1 / 30)) { timeline in
            art(flow: CGFloat(timeline.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 4) / 4))
        }
        .aspectRatio(250 / 170, contentMode: .fit)
        .accessibilityHidden(true)
    }

    private func art(flow: CGFloat) -> some View {
        Canvas { ctx, size in
            let s = size.width / 250
            func r(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat) -> CGRect { CGRect(x: x * s, y: y * s, width: w * s, height: h * s) }
            ctx.fill(Path(roundedRect: r(6, 6, 238, 134), cornerRadius: 12 * s), with: .color(Theme.bar))
            ctx.stroke(Path(roundedRect: r(6, 6, 238, 134), cornerRadius: 12 * s), with: .color(Theme.line3), lineWidth: 2 * s)
            ctx.fill(Path(roundedRect: r(16, 16, 218, 114), cornerRadius: 6 * s), with: .color(Theme.rgb(0x0F1A1C)))
            var wave = Path()
            wave.move(to: CGPoint(x: 24 * s, y: 82 * s))
            wave.addLine(to: CGPoint(x: 96 * s, y: 82 * s))
            wave.addCurve(to: CGPoint(x: 128 * s, y: 104 * s), control1: CGPoint(x: 110 * s, y: 82 * s), control2: CGPoint(x: 114 * s, y: 104 * s))
            wave.addCurve(to: CGPoint(x: 160 * s, y: 46 * s), control1: CGPoint(x: 144 * s, y: 104 * s), control2: CGPoint(x: 146 * s, y: 46 * s))
            wave.addCurve(to: CGPoint(x: 190 * s, y: 82 * s), control1: CGPoint(x: 174 * s, y: 46 * s), control2: CGPoint(x: 176 * s, y: 82 * s))
            wave.addLine(to: CGPoint(x: 226 * s, y: 82 * s))
            ctx.stroke(wave, with: .color(Theme.accent.opacity(0.18)), style: StrokeStyle(lineWidth: 3 * s, lineCap: .round, lineJoin: .round))
            ctx.stroke(wave, with: .color(Theme.accent), style: StrokeStyle(lineWidth: 3 * s, lineCap: .round, lineJoin: .round, dash: [40 * s, 220 * s], dashPhase: -flow * 260 * s))
            var stand = Path()
            stand.addLines([CGPoint(x: 104 * s, y: 140 * s), CGPoint(x: 98 * s, y: 160 * s), CGPoint(x: 152 * s, y: 160 * s), CGPoint(x: 146 * s, y: 140 * s)])
            ctx.fill(stand, with: .color(Theme.surface2))
            ctx.stroke(stand, with: .color(Theme.line3), lineWidth: 2 * s)
            ctx.fill(Path(roundedRect: r(84, 158, 82, 6), cornerRadius: 3 * s), with: .color(Theme.line2))
        }
    }
}
