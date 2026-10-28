// Ce qui se dessine par-dessus le passage et l'image (maquettes « App · Le
// passage » et « App · En session » ; docs/link-ui.md) : la capsule de la
// connexion (ses trois étapes, l'échec, la reconnexion, la fin), les rappels
// d'événements, la barre au survol (avec le menu « Ordinateurs » et « Garder
// le son », plusieurs ordinateurs). Dès la première image, rien ne s'affiche
// de lui-même : la barre et les statistiques viennent à la demande.

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
        // L'avis d'une demande (« Léa demande à rejoindre. ») prend les clics : il ouvre le panneau du partage.
        if case .request? = session.hint, !session.barVisible,
           session.hintRect.insetBy(dx: -4, dy: -4).contains(CGPoint(x: local.x, y: fromTop)) {
            return super.hitTest(point)
        }
        // La proposition « adapter à cet écran » prend les clics, le reste passe.
        if session.screenOfferShown, !session.barVisible,
           session.screenOfferRect.insetBy(dx: -4, dy: -4).contains(CGPoint(x: local.x, y: fromTop)) {
            return super.hitTest(point)
        }
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
    /// Repère de tous les rectangles mesurés (barre, fermeture des
    /// statistiques, capsule « adapter ») : la vue de session, origine en haut
    /// à gauche, comme le test de `ChromeHostingView.hitTest` et la position du
    /// pointeur. L'espace global de SwiftUI, pas un espace nommé sur la
    /// composition : celui-ci commence sous les zones de sécurité (barre de
    /// titre masquée en fenêtre, encoche en plein écran) et décalerait tout.
    static let space = CoordinateSpace.global

    private var streaming: Bool { session.phase == .streaming }

    var body: some View {
        ZStack(alignment: .top) {
            // Le trait de repère du bord haut n'est pas ici : il est tracé dans
            // l'image même (VideoRenderer), pour se voir sans couche SwiftUI.
            if !session.barVisible {
                VStack(spacing: 10) {
                    if let hint = session.hint {
                        if case let .request(text) = hint {
                            // Se choisit : il ouvre le panneau du partage (la barre, puis « Partager »).
                            Button { session.openSharePanel() } label: { HintToast(hint: .request(text)) }
                                .buttonStyle(.plain)
                                .background(GeometryReader { g in
                                    Color.clear.preference(key: HintRectKey.self, value: g.frame(in: Self.space))
                                })
                                .transition(.move(edge: .top).combined(with: .opacity))
                        } else {
                            HintToast(hint: hint)
                                .transition(.move(edge: .top).combined(with: .opacity))
                                .allowsHitTesting(false)
                        }
                    }
                    if streaming, session.screenOfferShown, let offer = session.screenOffer {
                        ScreenOfferToast(offer: offer, adapt: session.adaptToScreen, later: session.dismissScreenOffer)
                            .background(GeometryReader { g in
                                Color.clear.preference(key: ScreenOfferRectKey.self, value: g.frame(in: Self.space))
                            })
                            .transition(.move(edge: .top).combined(with: .opacity))
                    }
                }
                .padding(.top, 22)
            }
            if session.barVisible {
                SessionBar(session: session, sessions: AppModel.shared.sessions)
                    .padding(.top, 14)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
            PassageCapsule(session: session, sessions: AppModel.shared.sessions)
                .frame(maxHeight: .infinity, alignment: .bottom)
                .padding(.bottom, 92)
            if streaming, session.statsPanel {
                StatsPanel(stats: session.stats, history: session.statsHistory, audio: session.audioStats,
                           sessionLosses: session.sessionLosses, close: session.toggleStatsPanel, screens: session.screenCount,
                           route: session.path)
                    .padding(20)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                    .transition(.opacity.combined(with: .offset(x: 24)))
            }
        }
        // Toute la fenêtre : la barre en haut (dans la bande où la vue de
        // session laisse passer les clics), la capsule en bas, les mesures
        // en bas à droite.
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .onPreferenceChange(StatsCloseRectKey.self) { rect in session.statsCloseRect = rect }
        .onPreferenceChange(SessionBarRectKey.self) { rect in
            // Sans barre, la valeur retombe à zéro : la dernière mesure reste.
            if rect != .zero { session.barRect = rect }
        }
        .onPreferenceChange(GlassMenuOpenKey.self) { open in session.barMenuOpen = open }
        .onPreferenceChange(ScreenOfferRectKey.self) { rect in session.screenOfferRect = rect }
        .onPreferenceChange(HintRectKey.self) { rect in session.hintRect = rect }
        .animation(Theme.easeOut, value: session.screenOfferShown)
        .animation(Theme.easeOut, value: session.barVisible)
        .animation(Theme.easeOut, value: session.statsPanel)
        .preferredColorScheme(.dark)
    }
}

// MARK: - Capsule du passage

/// Ce qui se passe, en une ligne : les étapes de la connexion, l'échec, la
/// reconnexion, la fin.
private struct PassageCapsule: View {
    @ObservedObject var session: SessionController
    /// Les autres sessions : le bouton de l'échec (« Fermer » ou « Revenir à l'accueil »).
    @ObservedObject var sessions: SessionManager

    private enum Mode: Equatable { case waiting, reconnecting, failed, closing, hidden }

    private var mode: Mode {
        switch session.phase {
        // La connexion reprise en session (écrans ou profil changés) montre ses étapes.
        case .preparing, .connecting, .starting: return session.revealedAt == nil || session.replanning ? .waiting : .hidden
        case .reconnecting: return .reconnecting
        case .streaming: return .hidden
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
                if session.stationRestarting {
                    // Dernier recours : la Station redémarre, puis les étapes reprennent.
                    HStack(spacing: 10) {
                        ProgressView().controlSize(.small)
                        Text(StationText.restarting).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
                    }
                    .accessibilityElement(children: .combine)
                } else {
                    VStack(alignment: .leading, spacing: 7) {
                        // Un invité : les mêmes étapes, titrées « Invité sur {nom} ».
                        if session.isGuest {
                            Text(session.title).font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                                .padding(.bottom, 3)
                        }
                        step(0, session.expectedScreens > 1 ? String(localized: "Préparation des écrans") : String(localized: "Préparation de l'écran"))
                        step(1, String(localized: "Connexion"))
                        step(2, String(localized: "Premières images"))
                    }
                }
                divider
                Button("Annuler") { session.end() }
                    .buttonStyle(GhostButton(height: 34))
                    .keyboardShortcut(.cancelAction)
            case .reconnecting:
                if case let .reconnecting(n) = session.phase {
                    lines(title: String(localized: "Reconnexion à \(name)…"), sub: String(localized: "Essai \(n). L'image revient d'elle-même."))
                }
            case .failed:
                if case let .failed(message) = session.phase {
                    if session.shareEndText != nil {
                        // Fin par le partage : « {hôte} a arrêté le partage. », seule, sans « Reprendre la main ».
                        lines(title: message, sub: "")
                    } else {
                        lines(title: String(localized: "La connexion a échoué"), sub: message.prefix(1).uppercased() + message.dropFirst(), warn: true)
                    }
                }
                // Une session suivante (ou une autre encore ouverte) : « Fermer »
                // ferme sa fenêtre, les autres sessions continuent (docs/link-ui.md).
                if session.isFirst, sessions.all.allSatisfy({ $0 === session }) {
                    Button("Revenir à l'accueil") { session.dismissFailure() }
                        .buttonStyle(PrimaryButton(height: 40))
                } else {
                    Button("Fermer") { session.dismissFailure() }
                        .buttonStyle(PrimaryButton(height: 40))
                }
            case .closing:
                // Un invité n'a rien changé chez l'hôte : rien à rendre à l'ordinateur.
                if session.isGuest {
                    lines(title: String(localized: "À bientôt"), sub: "")
                } else {
                    lines(title: String(localized: "À bientôt"), sub: String(localized: "\(name) retrouve ses écrans et son son."))
                }
            case .hidden:
                EmptyView()
            }
        }
        .fixedSize()
    }

    /// Étape en cours : 0 (préparation de l'écran), 1 (connexion), 2 (premières images).
    private var current: Int {
        switch session.phase {
        case .preparing: return 0
        case .connecting: return 1
        default: return 2
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

    /// Une étape : faite (coche), en cours (points de suspension), à venir.
    private func step(_ index: Int, _ title: String) -> some View {
        let done = index < current
        let now = index == current
        return HStack(spacing: 9) {
            Group {
                if done {
                    Icon("check", size: 12, bold: true).foregroundStyle(Theme.ok)
                } else if now {
                    ProgressView().controlSize(.mini)
                } else {
                    Circle().strokeBorder(Theme.ghost, lineWidth: 1.2).frame(width: 8, height: 8)
                }
            }
            .frame(width: 14, height: 14)
            Text(now ? title + "…" : title)
                .font(Theme.sans(14, now ? .semibold : .regular))
                .foregroundStyle(done || now ? Theme.text : Theme.faint)
        }
        .accessibilityElement(children: .combine)
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

// MARK: - Écran de ce Mac changé

/// Capsule « L'écran a changé », dans le repère de la vue de session.
private struct ScreenOfferRectKey: PreferenceKey {
    static let defaultValue: CGRect = .zero
    static func reduce(value: inout CGRect, nextValue: () -> CGRect) {
        let next = nextValue()
        if next != .zero { value = next }
    }
}

/// L'avis d'une demande (« Léa demande à rejoindre. »), dans le repère de la vue de session.
private struct HintRectKey: PreferenceKey {
    static let defaultValue: CGRect = .zero
    static func reduce(value: inout CGRect, nextValue: () -> CGRect) {
        let next = nextValue()
        if next != .zero { value = next }
    }
}

/// « L'écran a changé : adapter l'image à 2560 × 1440 ? » [Adapter] [×]
private struct ScreenOfferToast: View {
    let offer: SessionController.ScreenMode
    let adapt: () -> Void
    let later: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Icon("monitor", size: 14, bold: true).foregroundStyle(Theme.accent)
            Text("L'écran a changé : adapter l'image à \(offer.width) × \(offer.height) ?")
            Button(action: adapt) {
                Text("Adapter")
                    .font(Theme.sans(12.5, .semibold))
                    .foregroundStyle(Theme.text)
                    .padding(.horizontal, 10)
                    .frame(height: 26)
                    .background(Theme.accentSoft, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Refaire l'écran de l'ordinateur à la définition et à la fréquence de cet écran (\(offer.refresh) Hz), puis relancer l'image")
            Button(action: later) {
                Icon("x", size: 12, bold: true)
                    .foregroundStyle(Theme.faint)
                    .frame(width: 24, height: 26)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Plus tard : la proposition reste dans la barre")
            .accessibilityLabel("Plus tard")
        }
        .font(Theme.sans(12.5))
        .foregroundStyle(Theme.text2)
        .padding(.leading, 14)
        .padding(.trailing, 6)
        .frame(height: 38)
        .glass(10, tint: 0.55)
    }
}

// MARK: - Rappel

private struct HintToast: View {
    let hint: SessionController.Hint

    var body: some View {
        HStack(spacing: 10) {
            switch hint {
            case .mouse:
                Text("Souris des jeux.")
                Keycap("⌃⌥⌘M")
                Text("pour reprendre le pointeur")
            case let .fallback(text):
                Icon("warning", size: 14, bold: true).foregroundStyle(Theme.warn)
                Text(text)
            case let .notice(text):
                Icon("monitor", size: 14, bold: true).foregroundStyle(Theme.accent)
                Text(text)
            case let .request(text):
                Icon("share-network", size: 14, bold: true).foregroundStyle(Theme.accent)
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

/// Rectangle de la barre, dans le repère de la vue de session : la zone
/// d'ouverture a sa largeur, et la barre se referme quand le pointeur quitte
/// ce rectangle.
private struct SessionBarRectKey: PreferenceKey {
    static let defaultValue: CGRect = .zero
    static func reduce(value: inout CGRect, nextValue: () -> CGRect) {
        let next = nextValue()
        if next != .zero { value = next }
    }
}

private struct SessionBar: View {
    @ObservedObject var session: SessionController
    /// Les sessions ouvertes : « Garder le son » à partir de deux.
    @ObservedObject var sessions: SessionManager

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
        guard choice == .auto else { return choice.label }
        return session.mouseCaptured ? MouseChoice.game.label : MouseChoice.desktop.label
    }

    private var audioLabel: String {
        guard session.host.settings.audio != .off else { return String(localized: "Son coupé") }
        switch session.audioStats?.channels {
        case 6?: return "5.1"
        case 8?: return "7.1"
        case 2?: return String(localized: "Stéréo")
        default: return session.host.settings.audio.label
        }
    }

    private var formatLabel: String {
        var parts = [session.modeLabel.replacingOccurrences(of: " · ", with: " · ")]
        if let s = session.stats, !s.format.isEmpty {
            // Un invité reçoit le format de son image (celui de l'hôte, ou un
            // format qu'il lit si le sien ne l'est pas), pas celui de ses réglages.
            let codec = session.imageCodec.rawValue.uppercased()
            parts.append("\(codec == "H264" ? "H.264" : codec) \(s.format.contains("4:4:4") ? "4:4:4" : "4:2:0")")
        }
        return parts.joined(separator: " · ")
    }

    var body: some View {
        VStack(spacing: 10) {
            // Une fenêtre étroite (une session de plus, aux deux tiers de
            // l'écran) : la barre compacte, sans le format ni le texte des
            // pastilles secondaires.
            ViewThatFits(in: .horizontal) {
                row(compact: false)
                row(compact: true)
            }
            .frame(height: 52)
            .glass(14, tint: 0.62)
            .background(GeometryReader { g in
                Color.clear.preference(key: SessionBarRectKey.self, value: g.frame(in: SessionChrome.space))
            })
            .shadow(color: .black.opacity(0.55), radius: 25, y: 14)

            if session.padMode {
                PadBanner().transition(.opacity.combined(with: .offset(y: -6)))
            }
        }
    }

    /// Les mesures en direct, qui ouvrent le panneau des statistiques.
    private func statsButton(compact: Bool) -> some View {
        Button { session.toggleStatsPanel() } label: {
            HStack(spacing: 10) {
                if let ms = session.stats?.latencyMs {
                    LitNumber(text: Moment.ms(ms))
                        .fixedSize()
                        .frame(minWidth: 58, alignment: .trailing)
                    Sparkline(values: session.latencyHistory)
                        .frame(width: 64, height: 22)
                } else if let fps = session.stats?.fps {
                    Text(String(localized: "\(Int(fps.rounded())) i/s")).font(Theme.mono(13)).foregroundStyle(Theme.text2)
                }
                if !compact {
                    Text(formatLabel).font(Theme.mono(11)).foregroundStyle(Theme.muted)
                }
            }
            .padding(.horizontal, 10)
            .frame(height: 36)
            .background(session.statsPanel ? Theme.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Theme.accentLine.opacity(session.statsPanel ? 1 : 0)))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(session.statsPanel ? "Fermer les statistiques (⌃⌥⌘S)" : "Statistiques (⌃⌥⌘S)")
        .accessibilityLabel("Statistiques")
    }

    /// Barre réduite d'un invité (docs/link-ui.md, « Partage ») : son rôle,
    /// « Quitter le partage », les mesures et le son ; la souris seulement si
    /// l'hôte lui a donné le clavier et la souris ; ni profil ni réglages.
    private func guestRow(compact: Bool) -> some View {
        let role = session.shareRole
        return HStack(spacing: 6) {
            Mark(size: 22, level: .lit)
            Text(session.title).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                .padding(.leading, 6).padding(.trailing, 8)
            divider
            chip(icon: ShareText.icon(role), text: ShareText.role(role), tint: role.plays ? Theme.accent : Theme.text2, compact: compact)
            relayChip(compact: compact)
            statsButton(compact: compact)
            divider
            keepSound(compact: compact)
            // « Qualité de l'image » : la qualité reçue, et les cinq choix.
            GlassMenu(items: ShareQuality.allCases.map { quality in
                .init(id: "\(quality.rawValue)", title: quality.label, checked: quality == session.shareQuality) { session.setShareQuality(quality) }
            }) {
                chip(icon: "monitor", text: session.shareImageLabel, caret: true, compact: compact)
            }
            .help(ShareText.imageQuality)
            .accessibilityLabel(Text(verbatim: "\(ShareText.imageQuality), \(session.shareImageLabel)"))
            .overlay(ring(0))
            if role.keyboardAndMouse {
                GlassMenu(items: MouseChoice.allCases.map { choice in
                    .init(id: "\(choice)", title: choice.label, checked: choice == session.host.settings.mouse) { session.setMouse(choice) }
                }) {
                    chip(icon: session.mouseCaptured ? "crosshair" : "cursor", text: mouseLabel, caret: true, compact: compact)
                }
                .help("Souris : Auto passe en souris des jeux quand un jeu est en plein écran sur l'ordinateur (⌃⌥⌘M bascule)")
            }
            divider
            if session.canToggleFullScreen {
                Button { session.toggleFullScreen() } label: {
                    Icon("corners-out", size: 16, bold: true)
                        .foregroundStyle(Theme.text2)
                        .frame(width: 38, height: 34)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Plein écran (⌃⌥⌘F)")
                .accessibilityLabel("Plein écran")
                .overlay(ring(1))
            }
            Button { session.end() } label: {
                HStack(spacing: 8) {
                    Icon("power", size: 15, bold: true)
                    Text(ShareText.leave).lineLimit(1).fixedSize()
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
            .help(ShareText.leave + " (⌃⌥⌘D)")
            .overlay(ring(session.padItems - 1))
        }
        .padding(.leading, 14)
        .padding(.trailing, 8)
        .fixedSize()
    }

    @ViewBuilder private func row(compact: Bool) -> some View {
        if session.isGuest { guestRow(compact: compact) } else { hostRow(compact: compact) }
    }

    private func hostRow(compact: Bool) -> some View {
        HStack(spacing: 6) {
            Mark(size: 22, level: .lit)
            Text(session.title).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                .padding(.leading, 6).padding(.trailing, 8)
            ComputersMenu(session: session, store: AppModel.shared.store, sessions: AppModel.shared.sessions, compact: compact)
            ShareBarButton(session: session, share: session.share, compact: compact)
            relayChip(compact: compact)
            divider
            statsButton(compact: compact)
            divider
            GlassMenu(items: Profile.allCases.map { profile in
                .init(id: profile.rawValue, title: profile.label, checked: profile == session.host.settings.profile) { session.select(profile) }
            }) {
                chip(icon: profileIcon, text: session.host.settings.profileLabel, tint: Theme.accent, caret: true)
            }
            .help("Profil : Jeu, Bureau ou Max")
            .overlay(ring(0))
            keepSound(compact: compact)
            GlassMenu(items: MouseChoice.allCases.map { choice in
                .init(id: "\(choice)", title: choice.label, checked: choice == session.host.settings.mouse) { session.setMouse(choice) }
            }) {
                chip(icon: session.mouseCaptured ? "crosshair" : "cursor", text: mouseLabel, caret: true, compact: compact)
            }
            .help("Souris : Auto passe en souris des jeux quand un jeu est en plein écran sur l'ordinateur (⌃⌥⌘M bascule)")
            .overlay(ring(1))
            chip(icon: "clipboard-text", text: String(localized: "Presse-papiers"), compact: compact)
                .opacity(session.host.settings.clipboard ? 1 : 0.45)
                .help(session.host.settings.clipboard ? "Presse-papiers partagé avec l'ordinateur (texte)" : "Presse-papiers non partagé (Réglages)")
            if session.canPickScreen {
                GlassMenu(items: session.screens.enumerated().map { i, screen in
                    .init(id: "\(screen.id)", title: String(localized: "Écran \(i + 1) · \(screen.width) × \(screen.height)"), checked: screen.id == session.shownDisplay) { session.showScreen(screen.id) }
                }) {
                    chip(icon: "monitor", text: String(localized: "Écran \((session.screens.firstIndex { $0.id == session.shownDisplay } ?? 0) + 1)"), caret: true)
                }
            }
            if let offer = session.screenOffer {
                Button { session.adaptToScreen() } label: {
                    chip(icon: "monitor", text: String(localized: "Adapter à \(offer.width) × \(offer.height)"), tint: Theme.accent)
                }
                .buttonStyle(.plain)
                .help("L'écran de ce Mac a changé : refaire l'écran de l'ordinateur à sa définition et relancer l'image")
            }
            if let offer = session.modeRestoreOffer {
                Button { session.restoreMode() } label: {
                    chip(icon: "arrow-counter-clockwise", text: offer, tint: Theme.accent)
                }
                .buttonStyle(.plain)
                .help("L'ordinateur a changé de définition : remettre celle de ce Mac")
            }
            divider
            // Plusieurs écrans : les fenêtres restent en plein écran, pas de « Plein écran ».
            if session.canToggleFullScreen {
                Button { session.toggleFullScreen() } label: {
                    Icon("corners-out", size: 16, bold: true)
                        .foregroundStyle(Theme.text2)
                        .frame(width: 38, height: 34)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Plein écran (⌃⌥⌘F)")
                .accessibilityLabel("Plein écran")
                .overlay(ring(2))
            }
            Button { session.end() } label: {
                HStack(spacing: 8) {
                    Icon("power", size: 15, bold: true)
                    Text("Se déconnecter").lineLimit(1).fixedSize()
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
            .overlay(ring(session.padItems - 1))
        }
        .padding(.leading, 14)
        .padding(.trailing, 8)
        // À sa taille, jamais tassée : la barre compacte prend le relais.
        .fixedSize()
    }

    /// Le haut-parleur : à partir de deux sessions ouvertes, « Garder le
    /// son » (plusieurs ordinateurs), allumé quand la session reste audible
    /// sans être active ; avec une seule session ou le son coupé dans les
    /// réglages, la pastille comme avant.
    @ViewBuilder private func keepSound(compact: Bool) -> some View {
        if session.host.settings.audio == .off || sessions.count < 2 {
            chip(icon: session.host.settings.audio == .off ? "speaker-slash" : "speaker-high", text: audioLabel, compact: compact)
        } else {
            let on = session.keepSound
            Button { AppModel.shared.sessions.setKeepSound(!on, for: session) } label: {
                chip(icon: "speaker-high", text: audioLabel, tint: on ? Theme.accent : Theme.text2, compact: compact)
                    .background(on ? Theme.accentSoft : .clear, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Theme.accentLine.opacity(on ? 1 : 0)))
            }
            .buttonStyle(.plain)
            .help("Garder le son")
            .accessibilityLabel("Garder le son")
            .accessibilityAddTraits(on ? .isSelected : [])
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

    /// « Par le relais » (docs/relais.md) : la session passe par le relais, hors
    /// de chez soi ; rien quand elle est directe.
    @ViewBuilder private func relayChip(compact: Bool) -> some View {
        if session.path.viaRelay {
            chip(icon: "globe-hemisphere-west", text: LinkText.viaRelay, tint: Theme.accent, compact: compact)
        }
    }

    /// `compact` : l'icône seule (barre étroite), le texte reste pour VoiceOver.
    private func chip(icon: String, text: String, tint: Color = Theme.text2, caret: Bool = false, compact: Bool = false) -> some View {
        HStack(spacing: 7) {
            Icon(icon, size: 16, bold: true).foregroundStyle(tint)
            if !compact {
                Text(text).font(Theme.sans(13)).foregroundStyle(Theme.text2).lineLimit(1).fixedSize()
            }
            if caret { Icon("caret-down", size: 10, bold: true).foregroundStyle(Theme.faint) }
        }
        .padding(.horizontal, compact ? 8 : 10)
        .frame(height: 34)
        .contentShape(Rectangle())
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(text))
    }
}

/// Plusieurs ordinateurs (docs/link-ui.md) : vos autres ordinateurs, dans
/// l'ordre de l'accueil, avec leur point et leur état comme dans le menu de
/// l'icône. Un ordinateur en ligne ouvre sa session (une fenêtre de plus), un
/// ordinateur hors ligne se réveille comme depuis l'accueil ; ceux déjà en
/// session sont cochés et passent devant (« Afficher »). Sans autre
/// ordinateur, pas de menu.
struct ComputersMenu: View {
    @ObservedObject var session: SessionController
    @ObservedObject var store: HostStore
    @ObservedObject var sessions: SessionManager
    /// Barre étroite : l'icône seule.
    var compact = false

    var body: some View {
        let items = Self.items(for: session, store: store, sessions: sessions)
        if !items.isEmpty {
            GlassMenu(items: items, onOpen: { Task { await store.refreshAll() } }) {
                HStack(spacing: 7) {
                    Icon("desktop-tower", size: 16, bold: true).foregroundStyle(Theme.text2)
                    if !compact {
                        Text("Ordinateurs").font(Theme.sans(13)).foregroundStyle(Theme.text2).lineLimit(1).fixedSize()
                    }
                    Icon("caret-down", size: 10, bold: true).foregroundStyle(Theme.faint)
                }
                .padding(.horizontal, compact ? 8 : 10)
                .frame(height: 34)
                .contentShape(Rectangle())
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("Ordinateurs")
            }
        }
    }

    /// Les lignes du menu : les autres ordinateurs, dans l'ordre de l'accueil
    /// (aussi lues par la démonstration, `bar-dump`).
    static func items(for session: SessionController, store: HostStore, sessions: SessionManager) -> [GlassMenuItem] {
        store.ordered.filter { $0.id != session.host.id }.map { item($0, store: store, sessions: sessions) }
    }

    private static func item(_ host: Host, store: HostStore, sessions: SessionManager) -> GlassMenuItem {
        let state = ComputerState(host, store: store)
        let id = host.id.uuidString
        if let other = sessions.session(for: host.id) {
            return .init(id: id, title: state.title, checked: true, dot: state.dot, detail: String(localized: "Afficher")) { other.bringToFront() }
        }
        if state.online == false {
            // La Station revient d'elle-même : rien à faire.
            if state.restarting || state.updating {
                return .init(id: id, title: state.title, dot: state.dot, enabled: false) {}
            }
            if state.waking {
                return .init(id: id, title: state.title, dot: state.dot, detail: LinkText.waking, enabled: false) {}
            }
            if store.canWake(host) {
                return .init(id: id, title: state.title, dot: state.dot, detail: LinkText.wake) { store.wake(host) }
            }
            return .init(id: id, title: state.title, dot: state.dot, detail: String(localized: "Réessayer")) {
                Task { await store.retry(host) }
            }
        }
        // En ligne (et « Recherche… », comme l'accueil) : une session de plus, quatre au plus.
        return .init(id: id, title: state.title, dot: state.dot, enabled: !sessions.isFull,
                     help: sessions.isFull ? LinkText.computersLimit : nil) { AppModel.shared.connect(host) }
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
                Text("La manette pilote KyberNow Link")
            }
            .font(Theme.sans(12, .semibold))
            .foregroundStyle(Theme.accentLight)
            Text("L'ordinateur ne la reçoit plus").font(Theme.sans(12)).foregroundStyle(Theme.faint)
            Rectangle().fill(Color.white.opacity(0.18)).frame(width: 1, height: 18)
            key("◀ ▶", String(localized: "naviguer"))
            key("A / ✕", String(localized: "choisir"))
            key("B / ○", String(localized: "fermer"))
            key("L1 R1 START SELECT", String(localized: "revenir à l'ordinateur"))
        }
        .padding(.horizontal, 16)
        .frame(height: 40)
        .glass(12, tint: 0.6)
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Theme.accent.opacity(0.4)))
    }
}
