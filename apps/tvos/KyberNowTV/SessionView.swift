// La session à l'écran : l'image de l'ordinateur, le passage et les étapes
// de la connexion (maquette « 4 · Le passage »), puis l'image seule ; le menu
// (« 5 · En session ») et ses rappels.
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
    private var connecting: Bool { [.preparing, .connecting, .starting].contains(session.phase) }

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
                Passage(host: session.host, open: revealed, title: session.title,
                        button: session.isBorrower ? LendText.borrow : session.isGuest ? ShareText.joinButton : String(localized: "Se connecter"))
                    .allowsHitTesting(false)
            }

            // La connexion : ses trois étapes et « Annuler », rien d'autre.
            if !revealed, connecting {
                ConnectingPanel(session: session)
                    .frame(maxHeight: .infinity, alignment: .bottom)
                    .padding(.bottom, 110)
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
                SessionFailure(host: session.host, title: session.title, message: message, afterImage: revealed,
                               shareEnd: session.shareEndText != nil || session.lendEndText != nil,
                               borrower: session.isBorrower, comeBack: session.canComeBack ? { session.comeBack() } : nil) { session.finish() }
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
    /// Le titre de la session (« Invité sur {nom} » pour un invité) et le bouton de l'accueil qu'il imite.
    var title: String?
    var button = String(localized: "Se connecter")
    @EnvironmentObject private var store: HostStore
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
    private func half(_ corner: Alignment) -> some View {
        ZStack(alignment: .topLeading) {
            HostBackdrop(host: host)
            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: 12) {
                    Circle().fill(Theme.ok).frame(width: 10, height: 10)
                    Text(LinkText.status(true)).font(Theme.sans(21))
                }
                .padding(.leading, 16).padding(.trailing, 18)
                .frame(height: 44)
                .glass(22)
                Text(title ?? host.name)
                    .font(Theme.display(140, 680))
                    .tracking(-140 * 0.045)
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                    .padding(.vertical, -16)
                if let os = store.status[host.id]?.info?.os {
                    Text(os).font(Theme.sans(24)).foregroundStyle(Theme.muted)
                }
                HStack(spacing: 14) {
                    Icon("monitor", size: 28).foregroundStyle(Theme.accent)
                    Text(HomeView.screenLine(host, info: store.status[host.id]?.info)).font(Theme.sans(24)).foregroundStyle(Theme.text2)
                }
                Text(button)
                    .font(Theme.sans(28, .semibold))
                    .padding(.horizontal, 36)
                    .frame(height: 80)
                    .background(SignalCorner(radius: 12, chamfer: 13).fill(Theme.accentFill))
                    .foregroundStyle(.white)
                    .neon(.lit)
                    .scaleEffect(1.06, anchor: .leading)
                    .padding(.top, 26)
            }
            .frame(width: 1080, alignment: .leading)
            .offset(x: 96, y: 176)
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

// MARK: - Connexion : les trois étapes

/// « Préparation de l'écran », « Connexion », « Premières images » et
/// « Annuler » (le bouton Retour annule aussi) ; « La Station redémarre… » à
/// leur place pendant le dernier recours de la connexion.
struct ConnectingPanel: View {
    @ObservedObject var session: SessionController
    @FocusState private var cancelFocused: Bool
    @State private var pulse = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// Étape en cours : 0, 1 ou 2.
    private var current: Int {
        switch session.phase {
        case .preparing: return 0
        case .connecting: return 1
        default: return 2
        }
    }

    /// Ordinateur utilisé : l'hôte doit accepter l'emprunt (« En attente de {hôte}… »).
    private var waitingHost: String? {
        guard session.phase == .preparing, case let .waiting(host)? = session.borrowFlow?.state else { return nil }
        return host
    }

    var body: some View {
        HStack(spacing: 34) {
            Mark(size: 48, level: .lit)
            VStack(alignment: .leading, spacing: 14) {
                if session.stationRestarting {
                    // Dernier recours : la Station redémarre, puis les étapes reprennent.
                    restartLine
                } else if let host = waitingHost {
                    // « Annuler » retire la demande chez l'hôte.
                    Text(session.title).font(Theme.sans(28, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                    HStack(spacing: 16) {
                        Circle().fill(Theme.accent).frame(width: 12, height: 12)
                            .shadow(color: Theme.accentFill, radius: 8)
                            .opacity(pulse ? 0.4 : 1)
                            .frame(width: 24, height: 24)
                        Text(LendText.waiting(host)).font(Theme.sans(24, .semibold)).foregroundStyle(Theme.text)
                    }
                } else {
                    // Un invité : les mêmes étapes, titrées « Invité sur {nom} » ; un ami : « Ordinateur de {hôte} ».
                    if session.isVisitor {
                        Text(session.title).font(Theme.sans(28, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                    }
                    // Un emprunt : le mode console se met en place d'abord.
                    step(0, session.isBorrower ? LendText.preparing : String(localized: "Préparation de l'écran"))
                    step(1, String(localized: "Connexion"))
                    step(2, String(localized: "Premières images"))
                }
            }
            .frame(minWidth: 420, alignment: .leading)
            Rectangle().fill(Color.white.opacity(0.12)).frame(width: 1)
            Button(String(localized: "Annuler")) { session.end() }
                .buttonStyle(ChoiceButton(selected: false, height: 64, light: true))
                .focused($cancelFocused)
        }
        .fixedSize(horizontal: false, vertical: true)
        .padding(.leading, 30).padding(.trailing, 34).padding(.vertical, 26)
        .glass(26, tint: 0.55)
        .onAppear {
            cancelFocused = true
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) { pulse = true }
        }
        .accessibilityElement(children: .contain)
    }

    private var restartLine: some View {
        HStack(spacing: 16) {
            Circle().fill(Theme.warn).frame(width: 12, height: 12)
                .shadow(color: Theme.warn, radius: 8)
                .opacity(pulse ? 0.4 : 1)
                .frame(width: 24, height: 24)
            Text(StationText.restarting).font(Theme.sans(24, .semibold)).foregroundStyle(Theme.text)
        }
        .frame(minHeight: 116)
    }

    private func step(_ i: Int, _ title: String) -> some View {
        HStack(spacing: 16) {
            ZStack {
                if i < current {
                    Icon("check", size: 22, bold: true).foregroundStyle(Theme.ok)
                } else if i == current {
                    Circle().fill(Theme.accent).frame(width: 12, height: 12)
                        .shadow(color: Theme.accentFill, radius: 8)
                        .opacity(pulse ? 0.4 : 1)
                } else {
                    Circle().strokeBorder(Theme.ghost, lineWidth: 1.6).frame(width: 12, height: 12)
                }
            }
            .frame(width: 24, height: 24)
            Text(i == current ? title + "…" : title)
                .font(Theme.sans(24, i == current ? .semibold : .regular))
                .foregroundStyle(i <= current ? Theme.text : Theme.faint)
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
    /// Le titre : le nom de l'ordinateur, ou « Invité sur {nom} ».
    var title: String?
    let message: String
    /// L'image était arrivée : la session s'est arrêtée en route.
    var afterImage = false
    /// Fin d'une session d'invité par le partage, ou d'un emprunt par le prêt :
    /// la raison seule, sans titre d'échec.
    var shareEnd = false
    /// Un ami qui emprunte : la carte de fin a « Fermer ».
    var borrower = false
    /// Réseau perdu pendant un emprunt : « Vous pouvez revenir pendant 15 min. » et « Revenir ».
    var comeBack: (() -> Void)?
    let done: () -> Void
    @FocusState private var focus: Int?

    var body: some View {
        ZStack {
            HostBackdrop(host: host, online: false)
            VStack(alignment: .leading, spacing: 26) {
                if !shareEnd {
                    HStack(spacing: 14) {
                        Icon("warning", size: 28).foregroundStyle(Theme.warn)
                        Theme.eyebrow(afterImage ? String(localized: "La session s'est arrêtée") : String(localized: "La connexion a échoué"), color: Theme.warnText).font(Theme.mono(18))
                    }
                }
                Text(title ?? host.name).font(Theme.display(110, 680)).tracking(-110 * 0.045).foregroundStyle(Theme.text)
                // La raison du cœur n'a pas de point final, sauf celle du partage (« {hôte} a arrêté le partage. »).
                Text(message.prefix(1).uppercased() + message.dropFirst() + (message.hasSuffix(".") ? "" : "."))
                    .font(Theme.sans(28)).foregroundStyle(Theme.text2)
                if let comeBack {
                    Text(LendText.comeBackHint).font(Theme.sans(26)).foregroundStyle(Theme.text)
                    HStack(spacing: 24) {
                        Button(LendText.comeBack, action: comeBack)
                            .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
                            .focused($focus, equals: 0)
                        Button(LendText.close, action: done)
                            .buttonStyle(ChoiceButton(selected: false, height: 80))
                    }
                    .padding(.top, 30)
                    .onAppear { focus = 0 }
                } else if borrower {
                    Button(LendText.close, action: done)
                        .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
                        .padding(.top, 30)
                } else {
                    Button("Retour à l'accueil", action: done)
                        .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
                        .padding(.top, 30)
                }
            }
            .frame(width: 1300, alignment: .leading)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(.leading, 96).padding(.top, 250)
        }
        .frame(width: 1920, height: 1080)
    }
}
