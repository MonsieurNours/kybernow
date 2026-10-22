// Manettes (maquette « 6 · Manettes ») : chaque manette est un joueur sur
// l'ordinateur ; test en direct des touches, sticks et gâchettes, et de la
// vibration, avant de jouer.

import KyberKit
import SwiftUI

struct GamepadsView: View {
    var inSession = false
    var onClose: (() -> Void)?
    @EnvironmentObject private var pads: GamepadBridge
    @EnvironmentObject private var app: AppModel
    @State private var selected: UInt32?
    @State private var shake = 0
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, slot(Int), rumble }

    private var pad: GamepadBridge.Pad? {
        pads.pads.first { $0.id == selected } ?? pads.pads.first
    }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            Button { close() } label: {
                HStack(spacing: 10) {
                    Icon("arrow-left", size: 26, bold: true)
                    Text("Retour").font(Theme.sans(22, .medium))
                }
                .padding(.leading, 18).padding(.trailing, 26)
                .frame(height: 60)
            }
            .buttonStyle(GlassButton(radius: 30))
            .focused($focus, equals: .back)
            .padding(.leading, 96).padding(.top, 56)

            players
                .frame(width: 620, alignment: .leading)
                .padding(.leading, 96).padding(.top, 170)

            live
                .frame(width: 1920 - 800 - 96, height: 1080 - 170 - 96)
                .glass(30)
                .padding(.leading, 800).padding(.top, 170)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, .slot(0))
        .onAppear { pads.monitoring = true }
        .onDisappear { pads.monitoring = false }
        .onExitCommand { close() }
    }

    private func close() {
        if let onClose { onClose() } else { app.screen = .home }
    }

    // MARK: Les joueurs

    private var players: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Manettes").font(Theme.display(64, 660)).tracking(-64 * 0.035).padding(.bottom, 6)
            Text("Chaque manette est un joueur sur l'ordinateur, dans l'ordre où elle s'est allumée.")
                .font(Theme.sans(22)).foregroundStyle(Theme.muted).lineSpacing(6)
                .padding(.bottom, 16)
            ForEach(0..<4, id: \.self) { i in
                slot(i)
            }
            if let waiting = pads.pads.first(where: \.waiting) {
                waitingRow(waiting)
            } else {
                remoteRow
            }
        }
        .focusSection()
    }

    /// Cinquième manette : XInput n'en expose que quatre à l'ordinateur.
    private func waitingRow(_ p: GamepadBridge.Pad) -> some View {
        let on = p.id == pad?.id
        let others = pads.pads.filter(\.waiting).count - 1
        return Button { selected = p.id } label: {
            HStack(spacing: 18) {
                Icon("game-controller", size: 32).foregroundStyle(Theme.warn)
                VStack(alignment: .leading, spacing: 4) {
                    (others > 0 ? Text("\(p.name) en attente (+ \(others))") : Text("\(p.name) en attente")).font(Theme.sans(22, .semibold))
                    Text("L'ordinateur ne voit que quatre manettes : elle prendra la place de la première qui se débranche")
                        .font(Theme.sans(17)).foregroundStyle(Theme.muted).lineLimit(2).fixedSize(horizontal: false, vertical: true)
                }
            }
            .padding(.horizontal, 26)
            .frame(width: 620, height: 84, alignment: .leading)
        }
        .buttonStyle(GlassButton(radius: 20, selected: on, scale: 1.04, fill: on ? Color.white.opacity(0.1) : Color.white.opacity(0.03)))
        .padding(.top, 6)
    }

    private var remoteRow: some View {
            HStack(spacing: 18) {
                Icon("television-simple", size: 32).foregroundStyle(Theme.muted)
                VStack(alignment: .leading, spacing: 4) {
                    Text(verbatim: "Siri Remote").font(Theme.sans(22, .semibold))
                    Text("Pilote KyberNow Link · en session, souris si vous l'activez").font(Theme.sans(17)).foregroundStyle(Theme.muted)
                }
            }
            .padding(.horizontal, 26)
            .frame(width: 620, height: 84, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 20, style: .continuous).fill(Color.white.opacity(0.03)))
            .overlay(RoundedRectangle(cornerRadius: 20, style: .continuous).strokeBorder(Color.white.opacity(0.06)))
            .padding(.top, 6)
    }

    private func slot(_ i: Int) -> some View {
        let p = pads.pads.first { $0.player == i + 1 }
        let on = p != nil && p?.id == pad?.id
        return Button { if let p { selected = p.id } } label: {
            HStack(spacing: 20) {
                Text("J\(i + 1)")
                    .font(Theme.mono(19, .semibold))
                    .foregroundStyle(p == nil ? Theme.faint : .white)
                    .frame(width: 56, height: 56)
                    .background(RoundedRectangle(cornerRadius: 14).fill(p == nil ? Color.clear : Theme.accentFill))
                    .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(p == nil ? Color.white.opacity(0.2) : .clear, lineWidth: 1.5))
                VStack(alignment: .leading, spacing: 6) {
                    (p.map { Text($0.name) } ?? Text("Place libre")).font(Theme.sans(25, .semibold)).foregroundStyle(p == nil ? Theme.faint : Theme.text)
                    (p != nil ? Text("Bluetooth · J\(i + 1) sur l'ordinateur") : i == 2 ? Text("Allumez une manette en mode appairage") : Text("Jusqu'à 4 joueurs sur l'ordinateur"))
                        .font(Theme.sans(18)).foregroundStyle(Theme.muted)
                }
                Spacer(minLength: 0)
                if let level = p?.battery {
                    HStack(spacing: 10) {
                        RoundedRectangle(cornerRadius: 6)
                            .strokeBorder(Color.white.opacity(0.45), lineWidth: 2)
                            .frame(width: 46, height: 22)
                            .overlay(alignment: .leading) {
                                RoundedRectangle(cornerRadius: 2)
                                    .fill(level < 0.5 ? Theme.warn : Theme.ok)
                                    .frame(width: 38 * CGFloat(level), height: 14)
                                    .padding(.leading, 4)
                            }
                        Text("\(Int((level * 100).rounded())) %").font(Theme.mono(17)).foregroundStyle(Theme.text2)
                    }
                }
            }
            .padding(.horizontal, 26)
            .frame(width: 620, height: 104)
        }
        .buttonStyle(GlassButton(radius: 20, selected: on, scale: 1.04, fill: on ? Color.white.opacity(0.1) : p == nil ? Color.clear : Color.white.opacity(0.04)))
        .focused($focus, equals: .slot(i))
        .disabled(p == nil)
    }

    // MARK: Le test en direct

    private var live: some View {
        let state = pad.flatMap { pads.live[$0.id] } ?? GamepadBridge.Live()
        let xbox = pad?.kind != .playstation
        let padLabel = pad?.label ?? "—"
        return VStack(alignment: .leading, spacing: 20) {
            HStack {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 12) {
                        Circle().fill(Theme.accent).frame(width: 9, height: 9).shadow(color: Theme.accentFill, radius: 6)
                        Theme.eyebrow(String(localized: "Test en direct · \(padLabel)"))
                    }
                    (pad.map { Text($0.name) } ?? Text("Aucune manette")).font(Theme.display(40, 640)).tracking(-1)
                }
                Spacer()
                Button {
                    if let pad { pads.testRumble(pad.id) }
                    shake += 1
                } label: {
                    HStack(spacing: 14) {
                        Icon("vibrate", size: 30).foregroundStyle(Theme.accentLight)
                        Text("Tester la vibration").font(Theme.sans(23, .semibold))
                    }
                    .padding(.leading, 24).padding(.trailing, 30)
                    .frame(height: 72)
                }
                .buttonStyle(GlassButton(radius: 14, fill: Theme.accentFill.opacity(0.2)))
                .focused($focus, equals: .rumble)
                .disabled(pad?.rumbles != true)
            }

            HStack(spacing: 30) {
                triggerColumn(xbox ? "LT" : "L2", state.leftTrigger, shoulder: xbox ? "LB" : "L1", pressed: state.pressed.contains("leftShoulder"), trailing: false)
                PadDrawing(state: state, xbox: xbox, player: pad?.player ?? 1)
                    .frame(width: 560, height: 326)
                    .modifier(Shake(animatableData: CGFloat(shake)))
                    .animation(.linear(duration: 0.7), value: shake)
                triggerColumn(xbox ? "RT" : "R2", state.rightTrigger, shoulder: xbox ? "RB" : "R1", pressed: state.pressed.contains("rightShoulder"), trailing: true)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            HStack(spacing: 20) {
                feature("Vibrations du jeu", "Moteurs gauche et droit, renvoyés par l'ordinateur")
                feature("Gâchettes analogiques", "Pression fine pour accélérer et freiner")
                feature(xbox ? "Voyant du joueur" : "Barre lumineuse",
                        xbox ? "La manette affiche J\(pad?.player ?? 1), comme sur l'ordinateur" : "Couleur du joueur ; le pavé tactile clique comme un bouton")
            }
        }
        .padding(.horizontal, 52).padding(.vertical, 44)
    }

    private func triggerColumn(_ name: String, _ value: Float, shoulder: String, pressed: Bool, trailing: Bool) -> some View {
        VStack(alignment: trailing ? .trailing : .leading, spacing: 14) {
            Text("\(name) · \(Int((value * 100).rounded())) %").font(Theme.mono(15)).foregroundStyle(Theme.muted)
            Capsule().fill(Color.white.opacity(0.1)).frame(height: 14)
                .overlay(alignment: .leading) {
                    Capsule().fill(Theme.accent).frame(width: 150 * CGFloat(value))
                }
            Text(shoulder).font(Theme.mono(15)).foregroundStyle(Theme.muted).padding(.top, 18)
            Capsule().fill(pressed ? Theme.accent : Color.white.opacity(0.1)).frame(height: 14)
                .shadow(color: pressed ? Theme.accentFill : .clear, radius: 8)
        }
        .frame(width: 150)
    }

    private func feature(_ title: LocalizedStringKey, _ detail: LocalizedStringKey) -> some View {
        HStack(alignment: .top, spacing: 14) {
            Icon("check", size: 24, bold: true).foregroundStyle(Theme.ok).padding(.top, 2)
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(Theme.sans(20, .semibold)).lineLimit(2).fixedSize(horizontal: false, vertical: true)
                Text(detail).font(Theme.sans(17)).foregroundStyle(Theme.muted).lineSpacing(4)
            }
            Spacer(minLength: 0)
        }
        .padding(20)
        .frame(maxWidth: .infinity, minHeight: 120, alignment: .topLeading)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
    }
}

/// La vibration d'essai secoue le dessin.
private struct Shake: GeometryEffect {
    var animatableData: CGFloat
    func effectValue(size: CGSize) -> ProjectionTransform {
        ProjectionTransform(CGAffineTransform(translationX: 5 * sin(animatableData * .pi * 16), y: 2 * sin(animatableData * .pi * 22)))
    }
}

/// Schéma de manette (générique), touches allumées en direct.
struct PadDrawing: View {
    let state: GamepadBridge.Live
    let xbox: Bool
    let player: Int

    private func on(_ name: String) -> Bool { state.pressed.contains(name) }

    var body: some View {
        // Repère de la maquette (740 × 430, origine en -20, 40), ramené à 640 × 400.
        ZStack(alignment: .topLeading) {
            Path { p in
                p.move(to: CGPoint(x: 150, y: 80))
                p.addLine(to: CGPoint(x: 550, y: 80))
                p.addCurve(to: CGPoint(x: 700, y: 250), control1: CGPoint(x: 640, y: 80), control2: CGPoint(x: 690, y: 150))
                p.addCurve(to: CGPoint(x: 620, y: 430), control1: CGPoint(x: 712, y: 360), control2: CGPoint(x: 680, y: 430))
                p.addCurve(to: CGPoint(x: 520, y: 340), control1: CGPoint(x: 570, y: 430), control2: CGPoint(x: 545, y: 380))
                p.addLine(to: CGPoint(x: 180, y: 340))
                p.addCurve(to: CGPoint(x: 80, y: 430), control1: CGPoint(x: 155, y: 380), control2: CGPoint(x: 130, y: 430))
                p.addCurve(to: CGPoint(x: 0, y: 250), control1: CGPoint(x: 20, y: 430), control2: CGPoint(x: -12, y: 360))
                p.addCurve(to: CGPoint(x: 150, y: 80), control1: CGPoint(x: 10, y: 150), control2: CGPoint(x: 60, y: 80))
                p.closeSubpath()
            }
            .fill(Theme.surface2)
            .overlay(
                Path { p in
                    p.move(to: CGPoint(x: 150, y: 80))
                    p.addLine(to: CGPoint(x: 550, y: 80))
                    p.addCurve(to: CGPoint(x: 700, y: 250), control1: CGPoint(x: 640, y: 80), control2: CGPoint(x: 690, y: 150))
                    p.addCurve(to: CGPoint(x: 620, y: 430), control1: CGPoint(x: 712, y: 360), control2: CGPoint(x: 680, y: 430))
                    p.addCurve(to: CGPoint(x: 520, y: 340), control1: CGPoint(x: 570, y: 430), control2: CGPoint(x: 545, y: 380))
                    p.addLine(to: CGPoint(x: 180, y: 340))
                    p.addCurve(to: CGPoint(x: 80, y: 430), control1: CGPoint(x: 155, y: 380), control2: CGPoint(x: 130, y: 430))
                    p.addCurve(to: CGPoint(x: 0, y: 250), control1: CGPoint(x: 20, y: 430), control2: CGPoint(x: -12, y: 360))
                    p.addCurve(to: CGPoint(x: 150, y: 80), control1: CGPoint(x: 10, y: 150), control2: CGPoint(x: 60, y: 80))
                    p.closeSubpath()
                }
                .stroke(Theme.rgb(0x3A4855), lineWidth: 3)
            )

            // Gâchettes hautes (L1, R1).
            key(on("leftShoulder"), RoundedRectangle(cornerRadius: 12), x: 110, y: 58, w: 150, h: 26)
            key(on("rightShoulder"), RoundedRectangle(cornerRadius: 12), x: 440, y: 58, w: 150, h: 26)
            // Sticks.
            stick(at: CGPoint(x: 190, y: 200), r: 52, knob: 30, value: state.leftStick, pressed: on("leftStick"))
            stick(at: CGPoint(x: 440, y: 300), r: 46, knob: 27, value: state.rightStick, pressed: on("rightStick"))
            // Croix.
            key(on("dpadUp"), Rectangle(), x: 252, y: 270, w: 24, h: 26)
            key(on("dpadDown"), Rectangle(), x: 252, y: 320, w: 24, h: 26)
            key(on("dpadLeft"), Rectangle(), x: 226, y: 296, w: 26, h: 24)
            key(on("dpadRight"), Rectangle(), x: 276, y: 296, w: 26, h: 24)
            Rectangle().fill(Color.white.opacity(0.08)).frame(width: 24, height: 24).offset(x: 252, y: 296)
            // Select, Start, Home.
            key(on("back"), Capsule(), x: 292, y: 150, w: 34, h: 18)
            key(on("start"), Capsule(), x: 374, y: 150, w: 34, h: 18)
            // Home : allumé s'il arrive à l'app (réglage « Bouton Guide » sur Jeu).
            key(on("guide"), Circle(), x: 330, y: 202, w: 40, h: 40)
            // Touches A B X Y (ou ✕ ○ □ △).
            face("y", x: 520, y: 128)
            face("x", x: 474, y: 174)
            face("b", x: 566, y: 174)
            face("a", x: 520, y: 220)
            // Voyant du joueur ou barre lumineuse.
            if xbox {
                HStack(spacing: 10) {
                    ForEach(1...4, id: \.self) { i in
                        Circle().fill(i == player ? Theme.accent : Color.white.opacity(0.12)).frame(width: 12, height: 12)
                    }
                }
                .offset(x: 310, y: 374)
            } else {
                RoundedRectangle(cornerRadius: 4).fill(Theme.accent).frame(width: 200, height: 8)
                    .shadow(color: Theme.accentFill, radius: 8)
                    .offset(x: 250, y: 86)
            }
        }
        .frame(width: 740, height: 430, alignment: .topLeading)
        .offset(x: 20, y: -40)
        .scaleEffect(560 / 740, anchor: .topLeading)
        .frame(width: 560, height: 326, alignment: .topLeading)
        .accessibilityElement()
        .accessibilityLabel("Schéma de la manette, touches en direct")
    }

    private func key<S: Shape>(_ pressed: Bool, _ shape: S, x: CGFloat, y: CGFloat, w: CGFloat, h: CGFloat) -> some View {
        shape.fill(pressed ? Theme.accent : Color.white.opacity(0.08))
            .overlay(shape.stroke(Color.white.opacity(0.28), lineWidth: 2))
            .shadow(color: pressed ? Theme.accentFill : .clear, radius: 8)
            .frame(width: w, height: h)
            .offset(x: x, y: y)
    }

    private func stick(at c: CGPoint, r: CGFloat, knob: CGFloat, value: CGPoint, pressed: Bool) -> some View {
        ZStack {
            Circle().fill(Theme.rgb(0x0A0F14)).overlay(Circle().stroke(Theme.line2, lineWidth: 2)).frame(width: r * 2, height: r * 2)
            Circle().fill(pressed ? Theme.accent : Theme.line2).overlay(Circle().stroke(Theme.ghost, lineWidth: 2))
                .frame(width: knob * 2, height: knob * 2)
                .offset(x: value.x * 18, y: -value.y * 18)
        }
        .frame(width: r * 2, height: r * 2)
        .offset(x: c.x - r, y: c.y - r)
    }

    private func face(_ name: String, x: CGFloat, y: CGFloat) -> some View {
        let pressed = on(name)
        return ZStack {
            Circle().fill(pressed ? Theme.accent : Color.white.opacity(0.08))
                .overlay(Circle().stroke(Color.white.opacity(0.28), lineWidth: 2))
                .shadow(color: pressed ? Theme.accentFill : .clear, radius: 8)
            if xbox {
                Text(name.uppercased()).font(Theme.sans(22, .heavy)).foregroundStyle(Theme.text)
            } else {
                psSymbol(name).foregroundStyle(Theme.text)
            }
        }
        .frame(width: 48, height: 48)
        .offset(x: x - 24, y: y - 24)
    }

    @ViewBuilder private func psSymbol(_ name: String) -> some View {
        switch name {
        case "y": Image(systemName: "triangle").font(.system(size: 18, weight: .bold))
        case "x": Image(systemName: "square").font(.system(size: 17, weight: .bold))
        case "b": Image(systemName: "circle").font(.system(size: 18, weight: .bold))
        default: Image(systemName: "xmark").font(.system(size: 17, weight: .bold))
        }
    }
}
