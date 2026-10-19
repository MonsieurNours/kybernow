// Ce qui se dessine par-dessus la scène et l'image : les textes de l'attente
// et de la fin, le rappel, la barre au survol (le décor animé est dans
// ConnectScene).

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
        return fromTop < session.barHeight ? super.hitTest(point) : nil
    }
}

struct SessionChrome: View {
    @ObservedObject var session: SessionController

    var body: some View {
        ZStack(alignment: .top) {
            WaitingLayer(session: session)
            if session.hint && !session.barVisible {
                HintToast()
                    .padding(.top, 20)
                    .transition(.move(edge: .top).combined(with: .opacity))
                    .allowsHitTesting(false)
            }
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

/// Textes de l'attente, sous le logo de la scène (ConnectScene dessine le
/// décor, les ondes, le halo et le logo).
private struct WaitingLayer: View {
    @ObservedObject var session: SessionController

    private var failed: Bool { if case .failed = session.phase { return true } else { return false } }
    private var hidden: Bool { session.revealedAt != nil && !failed }

    var body: some View {
        GeometryReader { geo in
            VStack(spacing: 10) {
                Text(session.host.name)
                    .font(Theme.display(30))
                    .foregroundStyle(Theme.text)
                ZStack {
                    status
                        .id(statusKey)
                        .transition(.asymmetric(insertion: .opacity.combined(with: .offset(y: 8)), removal: .opacity))
                }
                .font(Theme.sans(16))
                .foregroundStyle(Theme.muted)
                .animation(.easeOut(duration: 0.45), value: statusKey)
                if failed {
                    Button("Revenir à l'accueil") { session.dismissFailure() }
                        .buttonStyle(PrimaryButton())
                        .padding(.top, 14)
                }
            }
            .frame(width: geo.size.width)
            .padding(.top, geo.size.height / 2 + 62)
            .opacity(hidden ? 0 : 1)
            .animation(.easeOut(duration: 0.3), value: hidden)
            if !hidden && !failed {
                Button("Annuler") { session.end() }
                    .buttonStyle(.plain)
                    .font(Theme.sans(14))
                    .foregroundStyle(Theme.muted)
                    .frame(width: geo.size.width)
                    .position(x: geo.size.width / 2, y: geo.size.height - 56)
            }
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

/// Rappel après l'apparition.
private struct HintToast: View {
    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "arrow.up").font(.system(size: 12, weight: .bold)).foregroundStyle(Theme.accent)
            Text("Les commandes sont en haut de l'écran, au survol").font(Theme.sans(13)).foregroundStyle(Theme.text2)
        }
        .padding(.horizontal, 18)
        .frame(height: 44)
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(Color.white.opacity(0.14)))
    }
}

// MARK: - Barre au survol

private struct HoverBar: View {
    @ObservedObject var session: SessionController

    /// Anneau de focus en mode manette.
    private func ring(_ index: Int) -> some View {
        RoundedRectangle(cornerRadius: 10)
            .stroke(Theme.accent, lineWidth: 2)
            .shadow(color: Theme.accent.opacity(0.45), radius: 9)
            .opacity(session.padMode && session.padFocus == index ? 1 : 0)
            .animation(.easeOut(duration: 0.15), value: session.padFocus)
    }

    var body: some View {
        VStack(spacing: 10) {
            HStack(spacing: 8) {
                Circle().fill(Theme.accent).frame(width: 8, height: 8)
                Text(session.host.name).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                Chip(text: session.modeLabel)
                if let stats = session.stats {
                    Chip(text: String(format: "%.0f i/s · décodage %.1f ms", stats.fps, stats.decodeMs))
                        .help(statsHelp(stats, session.audioStats))
                }
                Divider().frame(height: 24).overlay(Theme.line)
                Menu {
                    ForEach(Profile.allCases) { profile in
                        Button {
                            session.select(profile)
                        } label: {
                            if profile == session.host.settings.profile {
                                Label(profile.label, systemImage: "checkmark")
                            } else {
                                Text(profile.label)
                            }
                        }
                    }
                } label: {
                    Text("Profil \(session.host.settings.profileLabel)").font(Theme.sans(13, .semibold))
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
                .padding(.horizontal, 10)
                .frame(height: 36)
                .overlay(ring(0))
                BarButton(symbol: "arrow.up.left.and.arrow.down.right", help: "Plein écran (⌃⌥⌘F)") { session.toggleFullScreen() }
                    .overlay(ring(1))
                Button {
                    session.end()
                } label: {
                    Label("Se déconnecter", systemImage: "rectangle.portrait.and.arrow.right")
                        .font(Theme.sans(13, .semibold))
                        .foregroundStyle(Theme.rgb(0xFF9B94))
                        .padding(.horizontal, 12)
                        .frame(height: 36)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Se déconnecter (⌃⌥⌘D)")
                .overlay(ring(2))
            }
            .padding(.leading, 16)
            .padding(.trailing, 8)
            .frame(height: 52)
            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16))
            .overlay(RoundedRectangle(cornerRadius: 16).stroke(Color.white.opacity(0.14)))
            .shadow(color: .black.opacity(0.45), radius: 22, y: 12)

            if session.padMode {
                PadBanner().transition(.opacity.combined(with: .offset(y: -6)))
            }
        }
    }
}

/// Rappel des touches quand la manette pilote la barre.
private struct PadBanner: View {
    private func key(_ k: String, _ label: String) -> some View {
        HStack(spacing: 5) {
            Text(k).font(Theme.mono(12, .medium)).foregroundStyle(Theme.text)
            Text(label).font(Theme.sans(12)).foregroundStyle(Theme.text2)
        }
    }

    var body: some View {
        HStack(spacing: 16) {
            Label("La manette pilote KyberNow", systemImage: "gamecontroller")
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
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Theme.accent.opacity(0.35)))
    }
}

/// Détail des mesures : image, son, et écart entre les deux.
private func statsHelp(_ v: VideoStats, _ a: AudioStats?) -> String {
    var text = "Image : \(v.width) × \(v.height), \(v.format). De la réception à l'écran : \(String(format: "%.1f", v.displayMs)) ms. Images remplacées par une plus récente : \(v.skipped)/s."
    if let a {
        let audio = a.bufferMs + a.outputMs
        text += "\nSon : \(a.channels) voies, tampon \(String(format: "%.0f", a.bufferMs)) ms, sortie \(String(format: "%.0f", a.outputMs)) ms."
        text += "\nLe son arrive \(String(format: "%.0f", audio - v.displayMs)) ms après l'image (imperceptible sous 45 ms)."
    }
    return text
}

private struct Chip: View {
    let text: String

    var body: some View {
        Text(text)
            .font(Theme.mono(12))
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

    private var trouble: Bool {
        session.closing.done && (session.closing.error != nil || session.closing.displayRestored == false || session.closing.audioRestored == false)
    }

    var body: some View {
        GeometryReader { geo in
            VStack(spacing: 22) {
                Text(session.closing.done && !trouble ? "À bientôt" : "Fin de la session")
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
                if trouble {
                    VStack(spacing: 12) {
                        Text("Rien n'est perdu : l'ordinateur rétablit aussi tout au prochain redémarrage de KyberNow.")
                            .font(Theme.sans(13))
                            .foregroundStyle(Theme.muted)
                        Button("Revenir à l'accueil") { session.finish() }
                            .buttonStyle(PrimaryButton())
                    }
                    .transition(.opacity)
                }
            }
            .frame(width: geo.size.width)
            .padding(.top, geo.size.height / 2 + 62)
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
            Text(text).font(Theme.sans(15)).foregroundStyle(ok ? Theme.text : Theme.warn)
        }
    }
}
