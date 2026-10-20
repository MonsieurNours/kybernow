// Ce qui se dessine par-dessus le passage et l'image (maquettes « App · Le
// passage » et « App · En session ») : la capsule qui dit ce qui se passe
// (préparation, entrée, échec, fin), le rappel, la barre au survol.

import AppKit
import KyberKit
import SwiftUI

/// Couche SwiftUI posée sur l'image : elle laisse passer clics et mouvements
/// vers l'ordinateur, sauf pendant l'attente, la fin, ou sur la barre ouverte.
final class ChromeHostingView: NSHostingView<SessionChrome> {
    weak var session: SessionController?

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let session else { return nil }
        if session.chromeCapturesAll { return super.hitTest(point) }
        let local = convert(point, from: superview)
        let fromTop = isFlipped ? local.y : bounds.height - local.y
        // Le panneau des mesures laisse tout passer, sauf son bouton de fermeture.
        if session.statsPanel, session.statsCloseRect.insetBy(dx: -4, dy: -4).contains(CGPoint(x: local.x, y: fromTop)) {
            return super.hitTest(point)
        }
        guard session.barVisible else { return nil }
        return fromTop < session.barHeight ? super.hitTest(point) : nil
    }
}

struct SessionChrome: View {
    @ObservedObject var session: SessionController
    /// Espace de coordonnées de la couche (origine en haut à gauche).
    static let space = "chrome"

    private var streaming: Bool { session.phase == .streaming }

    var body: some View {
        ZStack(alignment: .top) {
            // Zone du bord haut : la barre vit là.
            if streaming, !session.barVisible {
                Capsule().fill(Color.white.opacity(0.35)).frame(width: 220, height: 3)
                    .transition(.opacity)
                    .allowsHitTesting(false)
            }
            if let hint = session.hint, !session.barVisible {
                HintToast(hint: hint)
                    .padding(.top, 22)
                    .transition(.move(edge: .top).combined(with: .opacity))
                    .allowsHitTesting(false)
            }
            if session.barVisible {
                SessionBar(session: session)
                    .padding(.top, 14)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
            PassageCapsule(session: session)
                .frame(maxHeight: .infinity, alignment: .bottom)
                .padding(.bottom, 92)
            if streaming, session.statsPanel {
                StatsPanel(stats: session.stats, history: session.statsHistory, audio: session.audioStats,
                           sessionLosses: session.sessionLosses, close: session.toggleStatsPanel)
                    .padding(20)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                    .transition(.opacity.combined(with: .offset(x: 24)))
            }
        }
        // Toute la fenêtre : la barre en haut (dans la bande où la vue de
        // session laisse passer les clics), la capsule en bas, les mesures
        // en bas à droite.
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .coordinateSpace(name: Self.space)
        .onPreferenceChange(StatsCloseRectKey.self) { rect in session.statsCloseRect = rect }
        .animation(Theme.easeOut, value: session.barVisible)
        .animation(Theme.easeOut, value: session.statsPanel)
        .preferredColorScheme(.dark)
    }
}

// MARK: - Capsule du passage

/// Ce qui se passe, en une ligne : préparation, entrée, échec, fin.
private struct PassageCapsule: View {
    @ObservedObject var session: SessionController

    private enum Mode: Equatable { case waiting, entering, reconnecting, failed, closing, hidden }

    private var mode: Mode {
        switch session.phase {
        case .preparing, .connecting, .starting: return session.revealedAt == nil ? .waiting : .hidden
        case .reconnecting: return .reconnecting
        case .streaming: return session.entering ? .entering : .hidden
        case .failed: return .failed
        case .ending, .ended: return .closing
        }
    }

    var body: some View {
        Group {
            if mode != .hidden {
                content
                    .padding(.leading, 16).padding(.trailing, 20).padding(.vertical, 14)
                    .glass(16, tint: 0.55)
                    .shadow(color: .black.opacity(0.4), radius: 30, y: 16)
                    .transition(.opacity.combined(with: .offset(y: 14)))
            }
        }
        .animation(Theme.easeOut, value: mode)
        .animation(Theme.easeOut, value: session.closing)
    }

    private var name: String { session.host.name }

    @ViewBuilder private var content: some View {
        HStack(spacing: 18) {
            Mark(size: 34, level: mode == .failed || (mode == .closing && session.closing.done) ? .off : .lit)
            switch mode {
            case .waiting:
                lines(title: waitingTitle, sub: "Un écran de \(session.modeLabel.replacingOccurrences(of: " · ", with: " à ")) l'attend.")
                divider
                VStack(alignment: .leading, spacing: 5) {
                    check(session.phase != .preparing, "Écran \(session.modeLabel.replacingOccurrences(of: " · ", with: " à "))")
                    check(false, "Premières images")
                }
                divider
                Button("Annuler") { session.end() }
                    .buttonStyle(GhostButton(height: 34))
                    .keyboardShortcut(.cancelAction)
            case .entering:
                lines(title: "Vous entrez dans \(name)", sub: "Les commandes sont en haut de l'écran. ⌃⌥⌘F : fenêtre · ⌃⌥⌘D : quitter.")
                divider
                VStack(alignment: .leading, spacing: 5) {
                    check(true, "Écran \(session.modeLabel.replacingOccurrences(of: " · ", with: " à "))")
                    check(true, "Premières images")
                }
                latency
            case .reconnecting:
                if case let .reconnecting(n) = session.phase {
                    lines(title: "Reconnexion à \(name)…", sub: "Essai \(n). L'image revient d'elle-même.")
                }
            case .failed:
                if case let .failed(message) = session.phase {
                    lines(title: "Impossible d'entrer dans \(name)", sub: message.prefix(1).uppercased() + message.dropFirst(), warn: true)
                }
                Button("Revenir à l'accueil") { session.dismissFailure() }
                    .buttonStyle(PrimaryButton(height: 40))
            case .closing:
                lines(title: "À bientôt", sub: "\(name) retrouve ses écrans et son son.")
            case .hidden:
                EmptyView()
            }
        }
        .fixedSize()
    }

    private var waitingTitle: String {
        switch session.phase {
        case .preparing: return "On prépare \(name)"
        case .connecting: return "Connexion à \(name)…"
        default: return "Premières images…"
        }
    }

    private var divider: some View { Rectangle().fill(Color.white.opacity(0.12)).frame(width: 1, height: 38) }

    private func lines(title: String, sub: String, warn: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text)
                .contentTransition(.opacity)
            Text(sub).font(Theme.sans(13)).foregroundStyle(warn ? Theme.warnText : Theme.muted)
                .frame(maxWidth: 440, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func check(_ done: Bool, _ text: String) -> some View {
        HStack(spacing: 8) {
            Icon(done ? "check" : "dots-three", size: 12, bold: true)
                .foregroundStyle(done ? Theme.ok : Theme.faint)
            Text(text).font(Theme.mono(11)).foregroundStyle(done ? Theme.text2 : Theme.faint)
        }
    }

    @ViewBuilder private var latency: some View {
        if let ms = session.stats?.latencyMs {
            VStack(alignment: .trailing, spacing: 2) {
                LitNumber(text: Moment.ms(ms), size: 24)
                Theme.eyebrow("Écart")
            }
            .frame(minWidth: 104, alignment: .trailing)
        }
    }
}

/// Mesure en direct : chiffres allumés.
struct LitNumber: View {
    let text: String
    var size: CGFloat = 15

    var body: some View {
        Text(text)
            .font(Theme.mono(size, .medium))
            .foregroundStyle(Theme.accentLight)
            .shadow(color: Theme.neonCore.opacity(0.9), radius: 0.5)
            .shadow(color: Theme.accent.opacity(0.9), radius: 5)
            .shadow(color: Theme.accentFill.opacity(0.7), radius: 12)
            .contentTransition(.numericText())
    }
}

// MARK: - Rappel

private struct HintToast: View {
    let hint: SessionController.Hint

    var body: some View {
        HStack(spacing: 10) {
            switch hint {
            case .commands:
                Text("Les commandes sont en haut de l'écran, au survol.")
                Keycap("⌃⌥⌘F")
                Text("fenêtre")
            case .mouse:
                Text("Souris en mode jeu.")
                Keycap("⌃⌥⌘M")
                Text("pour la reprendre")
            case let .fallback(text):
                Icon("warning", size: 14, bold: true).foregroundStyle(Theme.warn)
                Text(text)
            case let .notice(text):
                Icon("monitor", size: 14, bold: true).foregroundStyle(Theme.accent)
                Text(text)
            }
        }
        .font(Theme.sans(12.5))
        .foregroundStyle(Theme.text2)
        .padding(.horizontal, 14)
        .frame(height: 38)
        .glass(10, tint: 0.55)
    }
}

struct Keycap: View {
    let keys: String
    init(_ keys: String) { self.keys = keys }

    var body: some View {
        Text(keys)
            .font(Theme.mono(11))
            .foregroundStyle(Theme.text)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .background(Color.white.opacity(0.08), in: RoundedRectangle(cornerRadius: 5))
    }
}

// MARK: - Barre de session

private struct SessionBar: View {
    @ObservedObject var session: SessionController

    /// Anneau de focus en mode manette.
    private func ring(_ index: Int) -> some View {
        RoundedRectangle(cornerRadius: 9)
            .stroke(Theme.accent, lineWidth: 2)
            .neon(.lit)
            .opacity(session.padMode && session.padFocus == index ? 1 : 0)
            .animation(.easeOut(duration: 0.15), value: session.padFocus)
    }

    private var mouseLabel: String {
        let choice = session.host.settings.mouse
        guard choice == .auto else { return choice == .game ? "Souris jeu" : "Souris" }
        return session.mouseCaptured ? "Souris jeu" : "Souris"
    }

    private var audioLabel: String {
        guard session.host.settings.audio != .off else { return "Son coupé" }
        switch session.audioStats?.channels {
        case 6?: return "5.1"
        case 8?: return "7.1"
        case 2?: return "Stéréo"
        default: return session.host.settings.audio.label
        }
    }

    private var formatLabel: String {
        var parts = [session.modeLabel.replacingOccurrences(of: " · ", with: " · ")]
        if let s = session.stats, !s.format.isEmpty {
            let codec = session.host.settings.codec.rawValue.uppercased()
            parts.append("\(codec == "H264" ? "H.264" : codec) \(s.format.contains("4:4:4") ? "4:4:4" : "4:2:0")")
        }
        return parts.joined(separator: " · ")
    }

    var body: some View {
        VStack(spacing: 10) {
            HStack(spacing: 6) {
                Mark(size: 22, level: .lit)
                Text(session.host.name).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                    .padding(.leading, 6).padding(.trailing, 8)
                divider
                Button { session.toggleStatsPanel() } label: {
                HStack(spacing: 10) {
                    if let ms = session.stats?.latencyMs {
                        LitNumber(text: Moment.ms(ms))
                            .frame(minWidth: 58, alignment: .trailing)
                        Sparkline(values: session.latencyHistory)
                            .frame(width: 64, height: 22)
                    } else if let fps = session.stats?.fps {
                        Text(String(format: "%.0f i/s", fps)).font(Theme.mono(13)).foregroundStyle(Theme.text2)
                    }
                    Text(formatLabel).font(Theme.mono(11)).foregroundStyle(Theme.muted)
                }
                .padding(.horizontal, 10)
                .frame(height: 36)
                .background(session.statsPanel ? Theme.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Theme.accentLine.opacity(session.statsPanel ? 1 : 0)))
                .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(session.statsPanel ? "Fermer les mesures détaillées (⌃⌥⌘S)" : "Mesures détaillées (⌃⌥⌘S)")
                .accessibilityLabel("Mesures détaillées")
                divider
                GlassMenu(items: Profile.allCases.map { profile in
                    .init(id: profile.rawValue, title: profile.label, checked: profile == session.host.settings.profile) { session.select(profile) }
                }) {
                    chip(icon: profileIcon, text: session.host.settings.profileLabel, tint: Theme.accent, caret: true)
                }
                .help("Profil : Jeu, Bureau ou Max")
                .overlay(ring(0))
                chip(icon: session.host.settings.audio == .off ? "speaker-slash" : "speaker-high", text: audioLabel)
                GlassMenu(items: MouseChoice.allCases.map { choice in
                    .init(id: "\(choice)", title: choice.label, checked: choice == session.host.settings.mouse) { session.setMouse(choice) }
                }) {
                    chip(icon: session.mouseCaptured ? "crosshair" : "cursor", text: mouseLabel, caret: true)
                }
                .help("Souris : Auto passe en mode jeu quand un jeu est en plein écran sur l'ordinateur (⌃⌥⌘M bascule)")
                .overlay(ring(1))
                chip(icon: "clipboard-text", text: "Presse-papiers")
                    .opacity(session.host.settings.clipboard ? 1 : 0.45)
                    .help(session.host.settings.clipboard ? "Presse-papiers partagé avec l'ordinateur (texte)" : "Presse-papiers non partagé (Réglages)")
                if session.canPickScreen {
                    GlassMenu(items: session.screens.enumerated().map { i, screen in
                        .init(id: "\(screen.id)", title: "Écran \(i + 1) · \(screen.width) × \(screen.height)", checked: screen.id == session.shownDisplay) { session.showScreen(screen.id) }
                    }) {
                        chip(icon: "monitor", text: "Écran \((session.screens.firstIndex { $0.id == session.shownDisplay } ?? 0) + 1)", caret: true)
                    }
                }
                if let offer = session.modeRestoreOffer {
                    Button { session.restoreMode() } label: {
                        chip(icon: "arrow-counter-clockwise", text: offer, tint: Theme.accent)
                    }
                    .buttonStyle(.plain)
                    .help("L'ordinateur a changé de définition : remettre celle de ce Mac")
                }
                divider
                Button { session.toggleFullScreen() } label: {
                    Icon("corners-out", size: 16, bold: true)
                        .foregroundStyle(Theme.text2)
                        .frame(width: 38, height: 34)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Plein écran ou fenêtre (⌃⌥⌘F)")
                .accessibilityLabel("Plein écran ou fenêtre")
                .overlay(ring(2))
                Button { session.end() } label: {
                    HStack(spacing: 8) {
                        Icon("power", size: 15, bold: true)
                        Text("Quitter")
                    }
                    .font(Theme.sans(13, .semibold))
                    .foregroundStyle(Theme.dangerText)
                    .padding(.horizontal, 14)
                    .frame(height: 36)
                    .background(Theme.danger.opacity(0.14), in: RoundedRectangle(cornerRadius: 9))
                    .overlay(RoundedRectangle(cornerRadius: 9).strokeBorder(Theme.danger.opacity(0.4)))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Se déconnecter (⌃⌥⌘D)")
                .overlay(ring(3))
            }
            .padding(.leading, 14)
            .padding(.trailing, 8)
            .frame(height: 52)
            .glass(14, tint: 0.62)
            .shadow(color: .black.opacity(0.55), radius: 25, y: 14)

            if session.padMode {
                PadBanner().transition(.opacity.combined(with: .offset(y: -6)))
            }
        }
    }

    private var profileIcon: String {
        switch session.host.settings.profile {
        case .jeu?: return "game-controller"
        case .bureau?: return "article"
        case .max?: return "lightning"
        case nil: return "sliders-horizontal"
        }
    }

    private var divider: some View { Rectangle().fill(Color.white.opacity(0.12)).frame(width: 1, height: 26) }

    private func chip(icon: String, text: String, tint: Color = Theme.text2, caret: Bool = false) -> some View {
        HStack(spacing: 7) {
            Icon(icon, size: 16, bold: true).foregroundStyle(tint)
            Text(text).font(Theme.sans(13)).foregroundStyle(Theme.text2)
            if caret { Icon("caret-down", size: 10, bold: true).foregroundStyle(Theme.faint) }
        }
        .padding(.horizontal, 10)
        .frame(height: 34)
        .contentShape(Rectangle())
    }
}

/// Courbe des latences des dernières secondes, avec le repère d'une image à 120 Hz.
private struct Sparkline: View {
    let values: [Double]

    var body: some View {
        Canvas { ctx, size in
            let budget = 1000.0 / 120
            let top = max(budget * 1.6, values.max() ?? budget)
            let y = { (v: Double) in size.height - CGFloat(v / top) * (size.height - 2) - 1 }
            var guide = Path()
            guide.move(to: CGPoint(x: 0, y: y(budget)))
            guide.addLine(to: CGPoint(x: size.width, y: y(budget)))
            ctx.stroke(guide, with: .color(Color.white.opacity(0.18)), style: StrokeStyle(lineWidth: 1, dash: [2, 3]))
            guard values.count > 1 else { return }
            var p = Path()
            for (i, v) in values.enumerated() {
                let pt = CGPoint(x: size.width * CGFloat(i) / CGFloat(max(1, values.count - 1)), y: y(v))
                if i == 0 { p.move(to: pt) } else { p.addLine(to: pt) }
            }
            ctx.stroke(p, with: .color(Theme.accent), style: StrokeStyle(lineWidth: 1.5, lineJoin: .round))
        }
        .accessibilityHidden(true)
    }
}

/// Rappel des touches quand la manette pilote la barre.
private struct PadBanner: View {
    private func key(_ k: String, _ label: String) -> some View {
        HStack(spacing: 5) {
            Text(k).font(Theme.mono(11, .medium)).foregroundStyle(Theme.text)
            Text(label).font(Theme.sans(12)).foregroundStyle(Theme.text2)
        }
    }

    var body: some View {
        HStack(spacing: 16) {
            HStack(spacing: 7) {
                Icon("game-controller", size: 16, bold: true)
                Text("La manette pilote KyberNow")
            }
            .font(Theme.sans(12, .semibold))
            .foregroundStyle(Theme.accentLight)
            Text("L'ordinateur ne la reçoit plus").font(Theme.sans(12)).foregroundStyle(Theme.faint)
            Rectangle().fill(Color.white.opacity(0.18)).frame(width: 1, height: 18)
            key("◀ ▶", "naviguer")
            key("A / ✕", "choisir")
            key("B / ○", "fermer")
            key("L1 R1 START SELECT", "revenir à l'ordinateur")
        }
        .padding(.horizontal, 16)
        .frame(height: 40)
        .glass(12, tint: 0.6)
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Theme.accent.opacity(0.4)))
    }
}
