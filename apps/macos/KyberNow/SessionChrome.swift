// Ce qui se dessine par-dessus l'image : l'attente, l'apparition (gerbe de
// particules et fondu), la barre au survol, la fin de session.

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
        guard session.barVisible else { return nil }
        let local = convert(point, from: superview)
        let fromTop = isFlipped ? local.y : bounds.height - local.y
        return fromTop < SessionController.barBand ? super.hitTest(point) : nil
    }
}

struct SessionChrome: View {
    @ObservedObject var session: SessionController

    var body: some View {
        ZStack(alignment: .top) {
            WaitingLayer(session: session)
            if session.barVisible {
                HoverBar(session: session)
                    .padding(.top, 14)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
            if session.phase == .ending || session.phase == .ended {
                ClosingLayer(session: session)
                    .transition(.opacity)
            }
        }
        .preferredColorScheme(.dark)
    }
}

// MARK: - Attente

private struct WaitingLayer: View {
    @ObservedObject var session: SessionController

    private var hidden: Bool {
        session.revealedAt != nil && session.phase != .preparing
            && !{ if case .failed = session.phase { return true } else { return false } }()
    }

    var body: some View {
        ZStack {
            Theme.background
                .opacity(hidden ? 0 : 1)
                .animation(.easeOut(duration: 1.2).delay(0.25), value: hidden)
            VStack(spacing: 18) {
                // Le logo s'efface en grandissant, puis quitte la scène (son
                // ondulation animée ne tourne plus sous l'image).
                if !hidden {
                    Mark(size: 110)
                        .transition(.scale(scale: 1.5).combined(with: .opacity))
                }
                Text(session.host.name)
                    .font(Theme.display(30))
                    .foregroundStyle(Theme.text)
                    .opacity(hidden ? 0 : 1)
                    .animation(.easeOut(duration: 0.3), value: hidden)
                status
                    .font(.system(size: 16))
                    .foregroundStyle(Theme.muted)
                    .opacity(hidden ? 0 : 1)
                    .animation(.easeOut(duration: 0.3), value: session.phase)
                    .id(statusKey)
                    .transition(.opacity.combined(with: .offset(y: 6)))
                if case .failed = session.phase {
                    Button("Revenir à l'accueil") { session.dismissFailure() }
                        .buttonStyle(PrimaryButton())
                        .padding(.top, 8)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .animation(.easeOut(duration: 0.5), value: hidden)
        }
        .allowsHitTesting(!hidden)
    }

    private var statusKey: String {
        switch session.phase {
        case .preparing: return "p"
        case .connecting: return "c"
        case .starting: return "s"
        case .reconnecting: return "r"
        case .failed: return "f"
        default: return "x"
        }
    }

    @ViewBuilder private var status: some View {
        switch session.phase {
        case .preparing: Text("On prépare votre écran…")
        case .connecting: Text("Connexion…")
        case .starting: Text("Presque prêt")
        case let .reconnecting(n): Text("Reconnexion (essai \(n))…")
        case let .failed(message): Text(message.prefix(1).uppercased() + message.dropFirst()).foregroundStyle(Theme.warn)
        default: Text(" ")
        }
    }
}

// MARK: - Barre au survol

private struct HoverBar: View {
    @ObservedObject var session: SessionController

    var body: some View {
        HStack(spacing: 8) {
            Circle().fill(Theme.accent).frame(width: 8, height: 8)
            Text(session.host.name).font(.system(size: 14, weight: .semibold)).foregroundStyle(Theme.text)
            Chip(text: session.modeLabel)
            Divider().frame(height: 24).overlay(Theme.line)
            Menu {
                ForEach(Profile.allCases) { profile in
                    Button {
                        session.select(profile)
                    } label: {
                        if profile == session.host.profile {
                            Label(profile.label, systemImage: "checkmark")
                        } else {
                            Text(profile.label)
                        }
                    }
                }
            } label: {
                Text("Profil \(session.host.profile.label)").font(.system(size: 13, weight: .semibold))
            }
            .menuStyle(.borderlessButton)
            .fixedSize()
            .padding(.horizontal, 10)
            BarButton(symbol: "arrow.up.left.and.arrow.down.right", help: "Plein écran (⌃⌥⌘F)") { session.toggleFullScreen() }
            Button {
                session.end()
            } label: {
                Label("Se déconnecter", systemImage: "rectangle.portrait.and.arrow.right")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Color(red: 1, green: 0.61, blue: 0.58))
                    .padding(.horizontal, 12)
                    .frame(height: 36)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Se déconnecter (⌃⌥⌘D)")
        }
        .padding(.leading, 16)
        .padding(.trailing, 8)
        .frame(height: 52)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(Color.white.opacity(0.14)))
        .shadow(color: .black.opacity(0.45), radius: 22, y: 12)
    }
}

private struct Chip: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 12, design: .monospaced))
            .foregroundStyle(Theme.accent)
            .padding(.horizontal, 10)
            .frame(height: 26)
            .background(Theme.accent.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
    }
}

private struct BarButton: View {
    let symbol: String
    let help: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(Theme.text)
                .frame(width: 36, height: 36)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(help)
    }
}

// MARK: - Fin de session

private struct ClosingLayer: View {
    @ObservedObject var session: SessionController

    var body: some View {
        ZStack {
            Theme.background.opacity(0.94)
            VStack(spacing: 24) {
                Mark(size: 92)
                Text(session.closing.done && session.closing.error == nil ? "À bientôt" : "Fin de la session")
                    .font(Theme.display(30))
                    .foregroundStyle(Theme.text)
                    .contentTransition(.opacity)
                VStack(alignment: .leading, spacing: 12) {
                    if let ok = session.closing.displayRestored {
                        Check(ok: ok, text: ok ? "L'écran virtuel est fermé, les écrans de l'ordinateur sont rallumés" : "Les écrans de l'ordinateur ne répondent pas encore")
                            .transition(.opacity.combined(with: .offset(y: 8)))
                    }
                    if let ok = session.closing.audioRestored {
                        Check(ok: ok, text: ok ? "Le son de l'ordinateur est revenu sur ses haut-parleurs" : "Le son de l'ordinateur n'est pas encore rétabli")
                            .transition(.opacity.combined(with: .offset(y: 8)))
                    }
                }
                .frame(minHeight: 60, alignment: .top)
                if session.closing.done, session.closing.error != nil || session.closing.displayRestored == false || session.closing.audioRestored == false {
                    VStack(spacing: 12) {
                        Text("Rien n'est perdu : l'ordinateur rétablit aussi tout au prochain redémarrage de KyberNow.")
                            .font(.system(size: 13))
                            .foregroundStyle(Theme.muted)
                        Button("Revenir à l'accueil") { session.finish() }
                            .buttonStyle(PrimaryButton())
                    }
                    .transition(.opacity)
                }
            }
        }
    }
}

private struct Check: View {
    let ok: Bool
    let text: String

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: ok ? "checkmark.circle.fill" : "exclamationmark.circle.fill")
                .font(.system(size: 18))
                .foregroundStyle(ok ? Theme.accent : Theme.warn)
            Text(text).font(.system(size: 15)).foregroundStyle(ok ? Theme.text : Theme.warn)
        }
    }
}
