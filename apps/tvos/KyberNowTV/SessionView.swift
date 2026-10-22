// La session à l'écran : l'image de l'ordinateur, le passage à l'arrivée
// (maquette « 4 · Le passage »), le menu (« 5 · En session ») et ses rappels.
//
// L'image vit dans un GCEventViewController : tant que le menu est fermé, la
// manette et la télécommande ne parlent qu'au jeu (le bouton Retour ne fait
// pas quitter l'app, il ouvre le menu) ; menu ouvert, elles pilotent KyberNow.

import AVKit
import GameController
import KyberKit
import SwiftUI

struct SessionView: View {
    @ObservedObject var session: SessionController
    @EnvironmentObject private var pads: GamepadBridge

    private var streaming: Bool { session.phase == .streaming || isReconnecting }
    private var isReconnecting: Bool { if case .reconnecting = session.phase { return true } else { return false } }
    private var revealed: Bool { session.revealedAt != nil }

    var body: some View {
        ZStack {
            Color.black
            // L'ordinateur, en direct ; il recule derrière le menu.
            // Aucun effet SwiftUI (flou, luminosité) sur la vue Metal : il la
            // fige sur une image. Le voile et le verre sont des calques par-dessus,
            // et seulement quand ils servent (passage, menu) : une couche
            // transparente laissée au-dessus de la couche Metal, même à
            // opacité 0, oblige tvOS à composer l'image au lieu de l'envoyer
            // telle quelle à l'écran, soit un balayage de plus (16,7 ms à 60 Hz).
            // Menu fermé, la couche Metal est seule : plein écran, opaque,
            // sans transformation ni coins arrondis.
            SessionStage(view: session.streamView, gameOnly: streaming && !session.menuOpen && session.phase != .ending,
                         scale: revealed ? (session.menuOpen ? 0.94 : 1) : 1.06, corner: session.menuOpen ? 28 : 0)
                .frame(width: 1920, height: 1080)
                .overlay {
                    if !revealed || session.menuOpen {
                        ZStack {
                            Rectangle().fill(.ultraThinMaterial)
                                .opacity(revealed ? (session.menuOpen ? 0.35 : 0) : 1)
                            Color.black.opacity(revealed ? (session.menuOpen ? 0.58 : 0) : 0.45)
                        }
                        .allowsHitTesting(false)
                        .transition(.opacity)
                    }
                }
                .animation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.5), value: session.menuOpen)
                .animation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.5), value: revealed)

            // Le passage : l'accueil s'ouvre en deux le long de la couture,
            // puis disparaît de la hiérarchie une fois ouvert (même raison).
            if session.phase != .ended, session.passageShown {
                Passage(host: session.host, open: revealed)
                    .allowsHitTesting(false)
            }

            if revealed, session.entering {
                EnteringCapsule(session: session)
                    .frame(maxHeight: .infinity, alignment: .bottom)
                    .padding(.bottom, 120)
                    .transition(.opacity.combined(with: .offset(y: 18)))
            }

            if session.menuHint, !session.menuOpen {
                MenuHint(text: String(localized: "Retour ou L1 + R1 + Start + Select : menu"))
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, 48)
                    .transition(.opacity)
            }

            if let note = session.notice, session.fallbackNote == nil, !session.menuOpen {
                MenuHint(text: note)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, 48)
                    .transition(.opacity)
            }

            if let note = session.fallbackNote, !session.menuOpen {
                MenuHint(text: note)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, 48)
                    .transition(.opacity)
            }

            // Manette en attente (déjà quatre joueurs), ou qui prend une place libérée.
            if let note = pads.notice, revealed, !session.menuOpen {
                MenuHint(text: note, glyph: false)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, 124)
                    .transition(.opacity)
            }

            if isReconnecting, !session.menuOpen {
                MenuHint(text: String(localized: "Reconnexion à \(session.host.name)…"))
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, 48)
            }

            if session.menuOpen {
                SessionMenu(session: session)
                    .transition(.opacity)
            }

            if case let .failed(message) = session.phase {
                SessionFailure(host: session.host, message: message) { session.finish() }
                    .transition(.opacity)
            }
        }
        .frame(width: 1920, height: 1080)
        .ignoresSafeArea()
        .onExitCommand {
            // Bouton Retour quand l'app a la main : il ferme le menu, ou annule l'attente.
            if session.menuOpen {
                session.closeMenu()
            } else if case .failed = session.phase {
                session.finish()
            } else if session.phase != .streaming {
                session.end()
            }
        }
    }
}

// MARK: - L'image (GCEventViewController)

struct SessionStage: UIViewControllerRepresentable {
    let view: StreamView
    /// La manette et la télécommande ne parlent qu'au jeu.
    let gameOnly: Bool
    /// Recul de l'image (menu, passage) et coins arrondis : animés par UIKit,
    /// sur la vue elle-même. Un effet SwiftUI sur une vue Metal la fige sur
    /// une copie de sa première image.
    let scale: CGFloat
    let corner: CGFloat

    func makeUIViewController(context: Context) -> GCEventViewController {
        let controller = GCEventViewController()
        let container = UIView(frame: CGRect(x: 0, y: 0, width: 1920, height: 1080))
        container.backgroundColor = .black
        view.frame = container.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.layer.cornerCurve = .continuous
        // Le découpage aux coins arrondis n'est posé que le temps du menu :
        // un masque sur la vue, même rectangulaire, empêche la présentation
        // directe de la couche Metal (voir SessionView).
        view.clipsToBounds = corner > 0
        container.addSubview(view)
        controller.view = container
        controller.controllerUserInteractionEnabled = !gameOnly
        apply(animated: false)
        return controller
    }

    func updateUIViewController(_ controller: GCEventViewController, context: Context) {
        controller.controllerUserInteractionEnabled = !gameOnly
        apply(animated: true)
    }

    private func apply(animated: Bool) {
        // Agrandissement autour d'un point à 30 % de la hauteur, comme dans la maquette.
        let anchorY = (0.3 - 0.5) * view.bounds.height
        let transform = CGAffineTransform(a: scale, b: 0, c: 0, d: scale, tx: 0, ty: anchorY * (1 - scale))
        guard view.transform != transform || view.layer.cornerRadius != corner else { return }
        let view = self.view
        if corner > 0 { view.clipsToBounds = true }
        let change = {
            view.transform = transform
            view.layer.cornerRadius = corner
        }
        // Coins revenus à zéro : le découpage est retiré, une fois l'animation finie.
        let settle = { (finished: Bool) in
            if finished, corner == 0, view.layer.cornerRadius == 0 { view.clipsToBounds = false }
        }
        if animated {
            UIView.animate(withDuration: scale == 1 && corner == 0 ? 1.2 : 0.5, delay: 0,
                           usingSpringWithDamping: 1, initialSpringVelocity: 0, options: [.beginFromCurrentState, .allowUserInteraction],
                           animations: change, completion: settle)
        } else {
            change()
            settle(true)
        }
    }
}

// MARK: - Le passage

/// Les deux moitiés de l'accueil (sa photographie : l'ordinateur, son nom,
/// le bouton allumé), le tube de néon de la couture, puis l'ouverture.
struct Passage: View {
    let host: Host
    let open: Bool
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var imagery: HostImagery
    @State private var tube = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            half(.topLeading)
                .offset(x: open ? -320 : 0, y: open ? -320 : 0)
            half(.bottomTrailing)
                .offset(x: open ? 320 : 0, y: open ? 320 : 0)
            // La couture : lumière et tube néon.
            ZStack {
                Rectangle()
                    .fill(EllipticalGradient(colors: [Theme.accentFill.opacity(0.45), Theme.accentFill.opacity(0)], center: .center, startRadiusFraction: 0, endRadiusFraction: 0.5))
                    .frame(width: 2300, height: 380)
                    .blendMode(.screen)
                ZStack {
                    Ellipse().fill(EllipticalGradient(colors: [Theme.accentFill.opacity(0.75), Theme.accentFill.opacity(0)], center: .center, startRadiusFraction: 0, endRadiusFraction: 0.5))
                        .frame(height: 108).blur(radius: 14)
                    Capsule().fill(Theme.accent).frame(height: 20).blur(radius: 6).opacity(0.95)
                    Capsule().fill(Theme.rgb(0xB9AFFF)).frame(height: 8)
                    Capsule().fill(Theme.neonCore).frame(height: 4).shadow(color: .white, radius: 5)
                }
                .frame(width: 1528)
                .mask(LinearGradient(stops: [.init(color: .clear, location: 0), .init(color: .black, location: 0.14),
                                             .init(color: .black, location: 0.86), .init(color: .clear, location: 1)],
                                     startPoint: .leading, endPoint: .trailing))
                .scaleEffect(x: tube ? 1 : 0.25, y: 1)
            }
            .rotationEffect(.degrees(-45))
            .opacity(open ? 0 : (tube ? 1 : 0))
        }
        .frame(width: 1920, height: 1080)
        .opacity(open ? 0 : 1)
        .blur(radius: open ? 12 : 0)
        .animation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.5), value: open)
        .onAppear {
            // Allumage du tube : grésillement, puis lumière franche.
            if reduceMotion { tube = true; return }
            withAnimation(.easeOut(duration: 0.08)) { tube = true }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.09) { withAnimation(.linear(duration: 0.05)) { tube = false } }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.16) { withAnimation(.easeOut(duration: 0.25)) { tube = true } }
        }
    }

    /// Comme sur l'accueil : la photographie de l'accueil s'ouvre.
    private var caption: String {
        if let p = imagery.pictures[host.id] {
            return p.source == .lastSession
                ? String(localized: "Dernière image · \(HomeView.when(p.date))")
                : String(localized: "Aperçu · \(HomeView.when(p.date))")
        }
        return host.lastSession.map { String(localized: "Dernière session · \(HomeView.when($0))") } ?? String(localized: "Jamais ouvert sur cette Apple TV")
    }

    private func half(_ corner: Alignment) -> some View {
        ZStack(alignment: .topLeading) {
            HostBackdrop(host: host)
            VStack(alignment: .leading, spacing: 22) {
                Theme.eyebrow(caption).font(Theme.mono(18))
                Text(host.name)
                    .font(Theme.display(140, 680))
                    .tracking(-140 * 0.045)
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                    .padding(.vertical, -16)
                HStack(spacing: 12) {
                    Circle().fill(Theme.ok).frame(width: 10, height: 10)
                    (app.isLocal(host) ? Text("En ligne · réseau local") : Text("En ligne · par VPN")).font(Theme.sans(21))
                }
                .padding(.leading, 16).padding(.trailing, 18)
                .frame(height: 44)
                .glass(22)
                Text("Se connecter")
                    .font(Theme.sans(28, .semibold))
                    .padding(.horizontal, 36)
                    .frame(height: 80)
                    .background(SignalCorner(radius: 12, chamfer: 13).fill(Theme.accentFill))
                    .foregroundStyle(.white)
                    .neon(.lit)
                    .scaleEffect(1.06, anchor: .leading)
                    .padding(.top, 80)
            }
            .frame(width: 1080, alignment: .leading)
            .offset(x: 96, y: 250)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .clipShape(HalfShape(corner: corner))
    }
}

/// Moitié de l'écran le long de la couture (45°, de (1500, 0) à (420, 1080)).
struct HalfShape: Shape {
    let corner: Alignment

    func path(in r: CGRect) -> Path {
        var p = Path()
        if corner == .topLeading {
            p.move(to: .zero)
            p.addLine(to: CGPoint(x: 1500, y: 0))
            p.addLine(to: CGPoint(x: 420, y: 1080))
            p.addLine(to: CGPoint(x: 0, y: 1080))
        } else {
            p.move(to: CGPoint(x: 1500, y: 0))
            p.addLine(to: CGPoint(x: 1920, y: 0))
            p.addLine(to: CGPoint(x: 1920, y: 1080))
            p.addLine(to: CGPoint(x: 420, y: 1080))
        }
        p.closeSubpath()
        return p
    }
}

// MARK: - Arrivée : ce qui se passe, en une ligne

struct EnteringCapsule: View {
    @ObservedObject var session: SessionController
    @EnvironmentObject private var pads: GamepadBridge

    var body: some View {
        HStack(spacing: 26) {
            Mark(size: 48, level: .lit)
            VStack(alignment: .leading, spacing: 6) {
                Text("Vous entrez dans \(session.host.name)").font(Theme.sans(25, .semibold))
                Text("Menu : bouton Retour de la télécommande, ou L1 + R1 + Start + Select.")
                    .font(Theme.sans(19)).foregroundStyle(Theme.muted)
            }
            Rectangle().fill(Color.white.opacity(0.12)).frame(width: 1)
            VStack(alignment: .leading, spacing: 7) {
                tick(String(localized: "Écran \(session.modeLabel)"))
                tick(soundLine)
                if let first = firstImage { tick(String(localized: "Premières images en \(first)")) }
            }
            .font(Theme.mono(15))
            .foregroundStyle(Theme.muted)
            VStack(alignment: .trailing, spacing: 4) {
                Text(session.stats?.latencyMs.map { "\(HomeView.ms($0)) ms" } ?? "—")
                    .font(Theme.mono(36, .medium))
                    .foregroundStyle(Theme.accentLight)
                    .shadow(color: Theme.neonCore, radius: 1)
                    .shadow(color: Theme.accent, radius: 10)
                    .shadow(color: Theme.accentFill, radius: 24)
                Text("ÉCART").font(Theme.mono(13)).tracking(1).foregroundStyle(Theme.faint)
            }
            .frame(minWidth: 150, alignment: .trailing)
        }
        .fixedSize(horizontal: false, vertical: true)
        .padding(.leading, 24).padding(.trailing, 30).padding(.vertical, 22)
        .glass(24, tint: 0.5)
    }

    private var soundLine: String {
        let audio = session.host.settings.audio == .off ? String(localized: "Son sur l'ordinateur") : String(localized: "Son \(session.route.channelsLabel)")
        let players = pads.pads.filter { !$0.waiting }.map { String(localized: "J\($0.player)") }
        switch players.count {
        case 0: return audio
        case 1: return String(localized: "\(audio) · \(players[0]) branché")
        default:
            let first = players.dropLast().joined(separator: ", ")
            let last = players.last!
            return String(localized: "\(audio) · \(first) et \(last) branchés")
        }
    }

    private var firstImage: String? {
        session.firstImageDelay.map {
            let s = String(format: "%.1f s", $0)
            return LanguageChoice.effective == "fr" ? s.replacingOccurrences(of: ".", with: ",") : s
        }
    }

    private func tick(_ text: String) -> some View {
        HStack(spacing: 10) {
            Icon("check", size: 16, bold: true).foregroundStyle(Theme.ok)
            Text(text)
        }
    }
}

struct MenuHint: View {
    let text: String
    var glyph = true

    var body: some View {
        HStack(spacing: 12) {
            if glyph { Glyph(kind: .remote, role: .back, size: 24) }
            Text(text).font(Theme.sans(20)).foregroundStyle(Theme.text2)
        }
        .padding(.horizontal, 28)
        .frame(height: 60)
        .glass(30, tint: 0.5)
    }
}

// MARK: - Échec

struct SessionFailure: View {
    let host: Host
    let message: String
    let done: () -> Void

    var body: some View {
        ZStack {
            HostBackdrop(host: host, online: false)
            VStack(alignment: .leading, spacing: 26) {
                HStack(spacing: 14) {
                    Icon("warning", size: 28).foregroundStyle(Theme.warn)
                    Theme.eyebrow(String(localized: "La session s'est arrêtée"), color: Theme.warnText).font(Theme.mono(18))
                }
                Text(host.name).font(Theme.display(110, 680)).tracking(-110 * 0.045).foregroundStyle(Theme.text)
                Text(message.prefix(1).uppercased() + message.dropFirst() + ".")
                    .font(Theme.sans(28)).foregroundStyle(Theme.text2)
                Button("Retour à l'accueil", action: done)
                    .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
                    .padding(.top, 30)
            }
            .frame(width: 1300, alignment: .leading)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(.leading, 96).padding(.top, 250)
        }
        .frame(width: 1920, height: 1080)
    }
}
