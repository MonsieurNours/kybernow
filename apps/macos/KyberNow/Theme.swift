// Thème KyberNow : sombre, menthe #6FE3C1, comme l'installateur et la console.

import SwiftUI

enum Theme {
    static let background = Color(red: 0x0D / 255, green: 0x11 / 255, blue: 0x17 / 255)
    static let surface = Color(red: 0x12 / 255, green: 0x19 / 255, blue: 0x22 / 255)
    static let surface2 = Color(red: 0x18 / 255, green: 0x20 / 255, blue: 0x2B / 255)
    static let line = Color(red: 0x2B / 255, green: 0x36 / 255, blue: 0x46 / 255)
    static let text = Color(red: 0xE9 / 255, green: 0xEE / 255, blue: 0xF5 / 255)
    static let muted = Color(red: 0x9A / 255, green: 0xA7 / 255, blue: 0xB8 / 255)
    static let accent = Color(red: 0x6F / 255, green: 0xE3 / 255, blue: 0xC1 / 255)
    static let accentInk = Color(red: 0x06 / 255, green: 0x25 / 255, blue: 0x1D / 255)
    static let accentSoft = Color(red: 0x1C / 255, green: 0x3A / 255, blue: 0x33 / 255)
    static let warn = Color(red: 0xF5 / 255, green: 0xB6 / 255, blue: 0x5A / 255)

    static func display(_ size: CGFloat) -> Font {
        .system(size: size, weight: .semibold, design: .rounded)
    }
}

/// Le « K » de KyberNow et ses ondes (même dessin que brand/kybernow-mark.svg).
struct Mark: View {
    var size: CGFloat = 44
    @State private var phase: CGFloat = 0

    var body: some View {
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
                ctx.stroke(wave, with: .color(Theme.accent.opacity(alpha)), style: StrokeStyle(lineWidth: 9 * s, lineCap: .round, lineJoin: .round, dash: [140 * s, 60 * s], dashPhase: -phase * s))
            }
            var arm = Path()
            arm.addLines([p(238, 264.5), p(404, 416), p(330, 416), p(206, 306)])
            ctx.fill(arm, with: .color(Color(white: 0.8)))
            var top = Path()
            top.addLines([p(206, 224), p(321, 96), p(392, 96), p(206, 302)])
            ctx.fill(top, with: .color(Color(white: 0.96)))
            ctx.fill(Path(roundedRect: CGRect(x: 150 * s, y: 96 * s, width: 56 * s, height: 320 * s), cornerRadius: 38 * s), with: .color(Color(white: 0.96)))
        }
        .frame(width: size, height: size)
        .onAppear {
            withAnimation(.linear(duration: 2.4).repeatForever(autoreverses: false)) { phase = 200 }
        }
    }
}

struct PrimaryButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        PrimaryLabel(configuration: configuration)
    }

    private struct PrimaryLabel: View {
        let configuration: Configuration
        @Environment(\.isEnabled) private var enabled

        var body: some View {
            configuration.label
                .font(.system(size: 15, weight: .semibold))
                .padding(.horizontal, 22)
                .frame(height: 44)
                .background(enabled ? Theme.accent.opacity(configuration.isPressed ? 0.8 : 1) : Theme.surface2, in: RoundedRectangle(cornerRadius: 11))
                .foregroundStyle(enabled ? Theme.accentInk : Theme.muted)
                .animation(.easeOut(duration: 0.2), value: enabled)
        }
    }
}

struct GhostButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 14))
            .padding(.horizontal, 16)
            .frame(height: 38)
            .background(configuration.isPressed ? Theme.surface2 : .clear, in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.line))
            .foregroundStyle(Theme.text)
    }
}
