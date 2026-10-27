// La session à l'écran, sur iPhone (toujours en paysage) et sur iPad
// (maquettes iOS, planches 3, 4, 5 et 8) : l'écran de connexion et ses trois
// étapes (docs/link-ui.md), le passage qui s'ouvre sur l'image, la languette
// et la barre, le bouton du clavier et sa rangée de touches, le menu Touches,
// les statistiques, les capsules des événements.
//
// Dès la première image, l'image seule : ni capsule d'arrivée, ni chiffre,
// ni barre qui s'ouvre d'elle-même ; les statistiques sont dans la barre, à
// la demande. Les capsules des événements (reconnexion, mode changé, manette
// branchée) sont toujours en haut de l'image, jamais une alerte modale.
//
// La manette à l'écran (ScreenPad.swift) se pose sur l'image, sous les
// capsules et la barre ; tant qu'elle est là, le bouton du clavier au pouce
// droit laisse sa place à A B X Y (le clavier reste dans la barre) et les
// capsules se resserrent entre ses gâchettes.

import GameController
import KyberKit
import SwiftUI
import UIKit

struct SessionView: View {
    @ObservedObject var session: SessionController
    @EnvironmentObject private var pads: GamepadBridge
    @State private var keyboardRow: KeyboardRow?

    private var revealed: Bool { session.revealedAt != nil }
    private var isReconnecting: Bool { if case .reconnecting = session.phase { return true } else { return false } }
    private var streaming: Bool { session.phase == .streaming }

    var body: some View {
        GeometryReader { g in
            ZStack {
                Color.black
                // L'ordinateur, en direct. Aucun effet SwiftUI (flou,
                // luminosité) sur la vue Metal : il la figerait sur une image.
                StreamStage(view: session.streamView)
                    .ignoresSafeArea()

                if session.phase != .ended {
                    Passage(host: session.host, open: revealed, title: session.title,
                            button: session.isGuest ? ShareText.joinButton : String(localized: "Se connecter"))
                        .allowsHitTesting(false)
                        .ignoresSafeArea()
                }

                if !revealed, session.phase != .ending, !isFailed {
                    ConnectingCard(session: session)
                        .frame(maxWidth: 420)
                        .frame(maxHeight: .infinity, alignment: .bottom)
                        .padding(.horizontal, 16)
                        .padding(.bottom, 24)
                        .transition(.opacity)
                }

                if streaming || isReconnecting {
                    chrome(size: g.size)
                }

                if case let .failed(message) = session.phase {
                    SessionFailure(host: session.host, title: session.title, message: message, connecting: !revealed,
                                   shareEnd: session.shareEndText != nil) { session.dismissFailure() }
                        .transition(.opacity)
                }
            }
        }
        .ignoresSafeArea()
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        .defersSystemGestures(on: .all)
        .sheet(isPresented: $session.keysMenu) {
            KeysMenu(session: session)
                .presentationDetents([.medium])
                .presentationBackground(.ultraThinMaterial)
        }
        // KyberNow Share, hôte : le panneau (code, QR code, invités).
        .sheet(isPresented: $session.shareSheet) {
            SharePanelView(session: session, share: session.share)
                .presentationDetents([.medium, .large])
                .presentationBackground(Theme.bar)
        }
        .sheet(isPresented: Binding(get: { session.statsSheet && !DeviceDisplay.isPad }, set: { session.statsSheet = $0 })) {
            SessionStatsView(session: session)
                .presentationDetents([.medium, .large])
                .presentationBackgroundInteraction(.enabled(upThrough: .medium))
                .presentationBackground(Theme.bar)
        }
        .onAppear {
            let row = KeyboardRow(session: session)
            keyboardRow = row
            session.streamView.keyboardAccessory = row.view
        }
        .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
            session.keyboardHidden()
        }
    }

    private var isFailed: Bool { if case .failed = session.phase { return true } else { return false } }

    private var padShown: Bool { streaming && session.screenPad }

    /// Languette, barre, capsules, clavier, statistiques.
    @ViewBuilder
    private func chrome(size: CGSize) -> some View {
        ZStack {
            // Pupitre : l'image est sur l'écran externe, l'appareil est le pavé.
            if session.external {
                ConsoleSurface(deviceIsPad: DeviceDisplay.isPad)
                    .allowsHitTesting(false)
                    .transition(.opacity)
            }
            if streaming, session.screenPad {
                let s = session.host.settings
                ScreenPadOverlay(layout: s.screenPadLayout, size: s.screenPadSize, opacity: s.screenPadOpacity,
                                 haptics: PadHaptics.available ? s.screenPadHaptics : .off,
                                 topReserved: session.external ? 76 : 0) { session.toggleScreenPadLayout() }
                    .ignoresSafeArea()
                    .transition(.opacity)
            }
            // Capsules en haut de l'image.
            VStack(spacing: 10) {
                if isReconnecting {
                    StateCapsule(icon: nil, neon: true, title: String(localized: "Connexion perdue · reconnexion à \(session.host.name)…"),
                                 detail: String(localized: "L'image reste figée ; la session reprend où elle était.")) {
                        Button("Se déconnecter") { session.end() }.buttonStyle(GlassButton(height: 44, fontSize: 14))
                    }
                } else if let note = session.fallbackNote {
                    StateCapsule(icon: "warning", tint: Theme.warn, title: note, detail: nil) { EmptyView() }
                } else if let note = session.notice {
                    StateCapsule(icon: nil, title: note, detail: nil) {
                        if let offer = session.modeRestoreOffer {
                            Button(offer) { session.restoreMode() }.buttonStyle(GlassButton(height: 44, fontSize: 14))
                        }
                    }
                } else if let pad = pads.notice {
                    StateCapsule(icon: "game-controller", title: pad, detail: nil) { EmptyView() }
                }
            }
            // Manette à l'écran : entre ses gâchettes, sous sa rangée du haut.
            .frame(maxWidth: padShown ? min(520, size.width * 0.6) : 620)
            .frame(maxHeight: .infinity, alignment: .top)
            .padding(.top, session.barOpen ? 80 : padShown && size.height < 600 ? 64 : 44)
            .padding(.horizontal, 16)
            .animation(Theme.easeOut, value: session.notice)

            if session.barOpen {
                SessionBar(session: session)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, 10)
                    .padding(.horizontal, 16)
                    .transition(.move(edge: .top).combined(with: .opacity))
            } else if streaming {
                Tab(session: session)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .transition(.opacity)
            }

            // Le bouton du clavier : pas pour un invité à qui l'hôte n'a pas donné le clavier et la souris.
            if streaming, !session.barOpen, !padShown, !session.isGuest || session.shareRole.keyboardAndMouse {
                HStack(alignment: .bottom) {
                    Spacer()
                    Button {
                        session.toggleKeyboard()
                    } label: {
                        Icon("keyboard", size: 22)
                            .foregroundStyle(Theme.text)
                            .frame(width: 52, height: 52)
                            .glass(26)
                    }
                    .accessibilityLabel(Text(session.keyboardShown ? "Fermer le clavier" : "Ouvrir le clavier"))
                }
                .frame(maxHeight: .infinity, alignment: .bottom)
                .padding(.horizontal, size.width > size.height ? 48 : 20)
                .padding(.bottom, 28)
            }

            // iPad : le panneau des statistiques, en bas à droite.
            if DeviceDisplay.isPad, session.statsSheet {
                SessionStatsView(session: session)
                    .frame(width: min(600, size.width - 48))
                    .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Theme.bar))
                    .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Theme.line))
                    .overlay(alignment: .topTrailing) {
                        Button { session.statsSheet = false } label: {
                            Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
                        }
                        .accessibilityLabel(Text("Fermer les statistiques"))
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                    .padding(24)
                    .transition(.opacity)
            }
        }
    }
}

// MARK: - L'image

/// La vue de session UIKit dans SwiftUI.
struct StreamStage: UIViewRepresentable {
    let view: StreamView
    func makeUIView(context: Context) -> StreamView { view }
    func updateUIView(_ uiView: StreamView, context: Context) {}
}

// MARK: - Le passage

/// Les deux moitiés de l'accueil (l'ordinateur, son nom, le bouton allumé),
/// le tube de néon de la couture à 45°, puis l'ouverture sur l'image. Même
/// passage que sur Mac et Apple TV.
struct Passage: View {
    let host: Host
    let open: Bool
    /// Le titre de la session (« Invité sur {nom} » pour un invité) et le bouton de l'accueil qu'il imite.
    var title: String?
    var button = String(localized: "Se connecter")
    @State private var tube = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        GeometryReader { g in
            let w = g.size.width, h = g.size.height
            let push = max(w, h) * 0.5
            ZStack {
                half(.topLeading, size: g.size)
                    .offset(x: open && !reduceMotion ? -push : 0, y: open && !reduceMotion ? -push : 0)
                half(.bottomTrailing, size: g.size)
                    .offset(x: open && !reduceMotion ? push : 0, y: open && !reduceMotion ? push : 0)
                // La couture : lumière et tube néon, le long de x + y = (w + h) / 2.
                ZStack {
                    Rectangle()
                        .fill(EllipticalGradient(colors: [Theme.accentFill.opacity(0.45), Theme.accentFill.opacity(0)], center: .center, startRadiusFraction: 0, endRadiusFraction: 0.5))
                        .frame(width: (w + h) * 1.1, height: min(w, h) * 0.6)
                        .blendMode(.screen)
                    ZStack {
                        Ellipse().fill(EllipticalGradient(colors: [Theme.accentFill.opacity(0.75), Theme.accentFill.opacity(0)], center: .center, startRadiusFraction: 0, endRadiusFraction: 0.5))
                            .frame(height: 84).blur(radius: 14)
                        Capsule().fill(Theme.accent).frame(height: 15).blur(radius: 5).opacity(0.95)
                        Capsule().fill(Theme.rgb(0xB9AFFF)).frame(height: 6)
                        Capsule().fill(Theme.neonCore).frame(height: 3).shadow(color: .white, radius: 4)
                    }
                    .frame(width: (w + h) * 0.72)
                    .mask(LinearGradient(stops: [.init(color: .clear, location: 0), .init(color: .black, location: 0.14),
                                                 .init(color: .black, location: 0.86), .init(color: .clear, location: 1)],
                                         startPoint: .leading, endPoint: .trailing))
                    .scaleEffect(x: tube ? 1 : 0.25, y: 1)
                }
                .rotationEffect(.degrees(-45))
                .position(x: w / 2, y: h / 2)
                .opacity(open ? 0 : (tube ? 1 : 0))
            }
            .opacity(open ? 0 : 1)
            .blur(radius: open && !reduceMotion ? 12 : 0)
            .animation(.timingCurve(0.2, 0.9, 0.1, 1, duration: reduceMotion ? 0.6 : 1.2), value: open)
        }
        .onAppear {
            // Allumage du tube : grésillement, puis lumière franche.
            if reduceMotion { tube = true; return }
            withAnimation(.easeOut(duration: 0.08)) { tube = true }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.09) { withAnimation(.linear(duration: 0.05)) { tube = false } }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.16) { withAnimation(.easeOut(duration: 0.25)) { tube = true } }
        }
    }

    private func half(_ corner: Alignment, size: CGSize) -> some View {
        let compact = size.height < 500
        return ZStack(alignment: .topLeading) {
            HostBackdrop(host: host)
            VStack(alignment: .leading, spacing: 12) {
                Text(title ?? host.name)
                    .font(Theme.display(compact ? 34 : 54, 680))
                    .tracking(-(compact ? 34 : 54) * 0.04)
                    .lineLimit(2)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                Text(button)
                    .font(Theme.sans(17, .semibold))
                    .padding(.horizontal, 20)
                    .frame(height: 56)
                    .background(SignalCorner(radius: 9, chamfer: 10).fill(Theme.accentFill))
                    .foregroundStyle(.white)
                    .neon(.lit)
            }
            .frame(maxWidth: 360, alignment: .leading)
            .padding(.leading, 24)
            .padding(.top, size.height * (compact ? 0.22 : 0.34))
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading)
        .clipShape(HalfShape(corner: corner))
    }
}

/// Moitié de l'écran le long de la couture (45°, par le centre).
struct HalfShape: Shape {
    let corner: Alignment

    func path(in r: CGRect) -> Path {
        let w = r.width, h = r.height
        let s = (w + h) / 2 // x + y = s
        var p = Path()
        // Points où la couture coupe les bords.
        let top = CGPoint(x: min(w, s), y: max(0, s - w))
        let bottom = CGPoint(x: max(0, s - h), y: min(h, s))
        if corner == .topLeading {
            p.move(to: .zero)
            p.addLine(to: CGPoint(x: top.x, y: 0))
            if top.y > 0 { p.addLine(to: top) }
            if bottom.x > 0 { p.addLine(to: bottom) }
            p.addLine(to: CGPoint(x: 0, y: bottom.y))
        } else {
            p.move(to: top.y > 0 ? top : CGPoint(x: top.x, y: 0))
            p.addLine(to: CGPoint(x: w, y: top.y))
            p.addLine(to: CGPoint(x: w, y: h))
            p.addLine(to: CGPoint(x: bottom.x, y: h))
            if bottom.x > 0 { p.addLine(to: bottom) } else { p.addLine(to: CGPoint(x: 0, y: bottom.y)) }
        }
        p.closeSubpath()
        return p
    }
}

// MARK: - Connexion

/// L'écran de connexion : ses trois étapes (préparation de l'écran par la
/// Station, connexion, premières images), rien d'autre, et « Annuler ».
/// Pendant le dernier recours du cœur (la Station redémarre, puis la
/// connexion repart), « La Station redémarre… » remplace les étapes.
struct ConnectingCard: View {
    @ObservedObject var session: SessionController

    /// Étape en cours : 0, 1 ou 2.
    private var current: Int {
        switch session.phase {
        case .preparing: return 0
        case .connecting: return 1
        default: return 2
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 14) {
                Mark(size: 30, level: .lit)
                if session.stationRestarting {
                    HStack(spacing: 10) {
                        HomeSpinner(size: 13).foregroundStyle(Theme.accentLight).frame(width: 16, height: 16)
                        Text(StationText.restarting)
                            .font(Theme.sans(15, .semibold))
                            .foregroundStyle(Theme.text)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .accessibilityElement(children: .combine)
                    .transition(.opacity)
                } else {
                    VStack(alignment: .leading, spacing: 9) {
                        // Un invité : les mêmes étapes, titrées « Invité sur {nom} ».
                        if session.isGuest {
                            Text(session.title).font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                        }
                        step(0, String(localized: "Préparation de l'écran"))
                        step(1, String(localized: "Connexion"))
                        step(2, String(localized: "Premières images"))
                    }
                    .transition(.opacity)
                }
                Spacer(minLength: 0)
            }
            Button("Annuler") { session.end() }
                .buttonStyle(GlassButton(height: 48, fontSize: 16))
                .frame(maxWidth: .infinity)
        }
        .padding(18)
        .glass(22, tint: 0.5)
        .animation(Theme.easeOut, value: current)
        .animation(Theme.easeOut, value: session.stationRestarting)
    }

    private func step(_ index: Int, _ title: String) -> some View {
        let done = index < current
        let now = index == current
        return HStack(spacing: 10) {
            ZStack {
                if done {
                    Icon("check", size: 12, bold: true).foregroundStyle(Theme.ok)
                } else if now {
                    HomeSpinner(size: 13).foregroundStyle(Theme.accentLight)
                } else {
                    Circle().strokeBorder(Theme.ghost, lineWidth: 1.4).frame(width: 9, height: 9)
                }
            }
            .frame(width: 16, height: 16)
            Text(now ? title + "…" : title)
                .font(Theme.sans(15, now ? .semibold : .regular))
                .foregroundStyle(done || now ? Theme.text : Theme.faint)
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(now ? .updatesFrequently : [])
    }
}

// MARK: - Languette et barre

/// La languette : la toucher ouvre la barre (et ses statistiques). Aucun
/// chiffre sur l'image tant qu'on ne les demande pas.
private struct Tab: View {
    @ObservedObject var session: SessionController

    var body: some View {
        Button { session.setBar(true) } label: {
            Image(systemName: "chevron.down").font(.system(size: 11, weight: .bold)).foregroundStyle(Theme.muted)
            .frame(width: 88, height: 26)
            .background(UnevenRoundedRectangle(bottomLeadingRadius: 14, bottomTrailingRadius: 14).fill(.ultraThinMaterial))
            .background(UnevenRoundedRectangle(bottomLeadingRadius: 14, bottomTrailingRadius: 14).fill(Theme.background.opacity(0.5)))
            .environment(\.colorScheme, .dark)
        }
        .accessibilityLabel(Text("Afficher la barre de session"))
    }
}

/// La barre : une ligne, tout au pouce, 44 pt par cible.
struct SessionBar: View {
    @ObservedObject var session: SessionController

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                Mark(size: 20, level: .lit).padding(.leading, 6)
                if DeviceDisplay.isPad {
                    Text(session.title).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text).padding(.horizontal, 6)
                    separator
                }
                statsButton
                separator
                if session.isGuest { guestRole } else { profileAndShare }
                item("speaker-high", session.host.settings.audio == .off ? String(localized: "Ordinateur") : session.route.name)
                // Le clavier et la souris : pas pour un invité à qui l'hôte ne les a pas donnés.
                if keyboardAndMouse { touchAndMouse }
                screenAndOffer
                if keyboardAndMouse { keysAndKeyboard }
                // La manette à l'écran : pour un invité, seulement quand il joue.
                if !session.isGuest || session.shareRole.plays { padButton }
                separator
                hideAndExit
            }
            .padding(.horizontal, 6)
        }
        .frame(height: 56)
        .glass(16, tint: 0.55)
        .overlay(alignment: .bottom) {
            if session.padMode {
                Text("Pendant la barre, la manette pilote KyberNow Link ; le jeu ne reçoit rien.")
                    .font(Theme.sans(12)).foregroundStyle(Theme.muted)
                    .offset(y: 28)
            }
        }
    }

    private var statsButton: some View {
        Button { session.statsSheet = true } label: {
            HStack(spacing: 8) {
                VStack(alignment: .trailing, spacing: 1) {
                    Text(session.stats?.latencyMs.map { Format.ms($0) } ?? "—")
                        .font(Theme.mono(14, .medium)).foregroundStyle(Theme.accentLight)
                    Text("Statistiques").font(Theme.sans(9.5)).foregroundStyle(Theme.text2).lineLimit(1)
                }
                .frame(minWidth: 58, alignment: .trailing)
                Sparkline(values: session.latencyHistory).frame(width: 52, height: 20)
            }
            .padding(.horizontal, 8)
            .frame(height: 44)
        }
        .accessibilityLabel(Text("Statistiques"))
    }

    /// Barre réduite d'un invité : son rôle ; ni profil ni réglages.
    private var guestRole: some View {
        item(session.shareRole.plays ? "game-controller" : "eye", ShareText.role(session.shareRole))
            .accessibilityElement(children: .combine)
    }

    @ViewBuilder private var profileAndShare: some View {
        Menu {
            ForEach(Profile.allCases) { profile in
                Button { session.select(profile) } label: {
                    if session.host.settings.profile == profile { Label(profile.label, systemImage: "checkmark") } else { Text(profile.label) }
                }
            }
        } label: {
            item("sliders-horizontal", session.host.settings.profileLabel)
        }
        shareItem
    }

    @ViewBuilder private var touchAndMouse: some View {
        Picker("Toucher", selection: Binding(get: { session.directTouch }, set: { session.setDirectTouch($0) })) {
            Text("Pavé").tag(false)
            Text("Direct").tag(true)
        }
        .pickerStyle(.segmented)
        .frame(width: 168)
        .accessibilityLabel(Text("Toucher"))
        Menu {
            ForEach(MouseChoice.allCases, id: \.self) { choice in
                Button { session.setMouse(choice) } label: {
                    if session.host.settings.mouse == choice { Label(choice.label, systemImage: "checkmark") } else { Text(choice.label) }
                }
            }
        } label: {
            item("cursor", mouseLabel)
        }
    }

    private var mouseLabel: String {
        let choice = session.host.settings.mouse
        return choice == .auto ? String(localized: "Souris · \(choice.label)") : choice.label
    }

    @ViewBuilder private var screenAndOffer: some View {
        if session.canPickScreen, !session.isGuest {
            Menu {
                ForEach(session.screens, id: \.id) { d in
                    Button("\(d.width) × \(d.height)") { session.showScreen(d.id) }
                }
            } label: {
                item("monitor", String(localized: "Écran"))
            }
        }
        if let offer = session.modeRestoreOffer {
            Button { session.restoreMode() } label: { item("arrows-clockwise", offer) }
        }
    }

    @ViewBuilder private var keysAndKeyboard: some View {
        Button { session.keysMenu = true } label: { item("keyboard", String(localized: "Touches")) }
        Button { session.toggleKeyboard() } label: { item("keyboard", String(localized: "Clavier")) }
            .accessibilityLabel(Text(session.keyboardShown ? "Fermer le clavier" : "Ouvrir le clavier"))
    }

    private var padButton: some View {
        Button { session.toggleScreenPad() } label: {
            item("game-controller", ScreenPadText.name)
                .background {
                    if session.screenPad {
                        RoundedRectangle(cornerRadius: 9, style: .continuous)
                            .fill(Color.white.opacity(0.12))
                            .overlay(alignment: .bottom) { Rectangle().fill(Theme.accent).frame(height: 2) }
                            .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
                    }
                }
        }
        .accessibilityLabel(Text(ScreenPadText.name))
        .accessibilityValue(session.screenPad ? Text("Activé") : Text("Désactivé"))
        .accessibilityAddTraits(session.screenPad ? .isSelected : [])
    }

    @ViewBuilder private var hideAndExit: some View {
        Button { session.setBar(false) } label: {
            Image(systemName: "chevron.up").font(.system(size: 15, weight: .bold)).foregroundStyle(Theme.text2).frame(width: 40, height: 44)
        }
        .accessibilityLabel(Text("Masquer la barre"))
        Button { session.end() } label: {
            HStack(spacing: 8) {
                Icon("power", size: 18)
                // Un invité quitte le partage ; il n'éteint rien chez l'hôte.
                if session.isGuest { Text(ShareText.leave).font(Theme.sans(13, .semibold)).lineLimit(1).fixedSize() }
            }
            .foregroundStyle(Theme.dangerText)
            .padding(.horizontal, session.isGuest ? 12 : 0)
            .frame(minWidth: 44, minHeight: 44)
            .background(RoundedRectangle(cornerRadius: 10).fill(Theme.danger.opacity(0.14)))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Theme.danger.opacity(0.4)))
        }
        .accessibilityLabel(Text(session.isGuest ? ShareText.leave : String(localized: "Se déconnecter")))
    }

    private var separator: some View {
        Rectangle().fill(Color.white.opacity(0.12)).frame(width: 1, height: 28)
    }

    /// Le clavier et la souris de l'appareil partent vers l'ordinateur : sa
    /// session, ou un invité à qui l'hôte les a donnés.
    private var keyboardAndMouse: Bool { !session.isGuest || session.shareRole.keyboardAndMouse }

    /// « Partager » (KyberNow Share, hôte) : ouvre le partage et son panneau ;
    /// inactif avec la raison quand l'ordinateur ne le permet pas. La pastille
    /// « 2 invités » rouvre le panneau.
    @ViewBuilder private var shareItem: some View {
        let availability = session.shareAvailability
        let count = session.share.guests.count
        let label = count > 0 ? ShareText.share + ", " + ShareText.guestCount(count) : ShareText.share
        // Inactif (estompé) : le panneau dit pourquoi, faute d'infobulle au toucher.
        Button {
            session.shareSheet = true
            if availability.isAvailable { Task { await session.share.openIfNeeded() } }
        } label: {
            HStack(spacing: 2) {
                item("share-network", ShareText.share)
                if count > 0 { guestBadge(count) }
            }
            .background { shareHighlight }
        }
        .opacity(availability.isAvailable ? 1 : 0.45)
        .accessibilityLabel(Text(verbatim: label))
        .accessibilityHint(Text(verbatim: availability.reason ?? ""))
    }

    /// La pastille « 2 invités ».
    private func guestBadge(_ count: Int) -> some View {
        Text(ShareText.guestCount(count))
            .font(Theme.sans(11.5, .semibold))
            .foregroundStyle(Theme.text)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 8)
            .frame(height: 22)
            .background(Capsule().fill(Theme.accentFill.opacity(0.28)))
            .overlay(Capsule().strokeBorder(Theme.accent.opacity(0.6)))
    }

    /// « Partager » allumé tant que le partage est ouvert.
    @ViewBuilder private var shareHighlight: some View {
        if session.share.isOpen {
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .fill(Color.white.opacity(0.12))
                .overlay(alignment: .bottom) { Rectangle().fill(Theme.accent).frame(height: 2) }
                .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
        }
    }

    private func item(_ icon: String, _ text: String) -> some View {
        VStack(spacing: 2) {
            Icon(icon, size: 18).foregroundStyle(Theme.accentLight)
            Text(text).font(Theme.sans(9.5)).foregroundStyle(Theme.text2).lineLimit(1)
        }
        .frame(minWidth: 48, minHeight: 44)
        .padding(.horizontal, 4)
    }
}

/// Courbe de latence des dernières secondes.
struct Sparkline: View {
    let values: [Double]

    var body: some View {
        Canvas { ctx, size in
            guard values.count > 1 else { return }
            let lo = (values.min() ?? 0) * 0.9, hi = max((values.max() ?? 1) * 1.1, lo + 1)
            var p = Path()
            for (i, v) in values.enumerated() {
                let x = size.width * CGFloat(i) / CGFloat(values.count - 1)
                let y = size.height * (1 - CGFloat((v - lo) / (hi - lo)))
                if i == 0 { p.move(to: CGPoint(x: x, y: y)) } else { p.addLine(to: CGPoint(x: x, y: y)) }
            }
            ctx.stroke(p, with: .color(Theme.accent), style: StrokeStyle(lineWidth: 1.5, lineJoin: .round))
        }
        .accessibilityHidden(true)
    }
}

// MARK: - Pupitre

/// L'appareil quand l'image est sur l'écran externe : toute la surface est le
/// pavé tactile ; une trame discrète et ce qu'on peut y faire.
private struct ConsoleSurface: View {
    let deviceIsPad: Bool

    var body: some View {
        ZStack {
            Theme.background
            // Trame du pavé : des points très discrets.
            Canvas { ctx, size in
                let step: CGFloat = 28
                var y: CGFloat = step / 2
                while y < size.height {
                    var x: CGFloat = step / 2
                    while x < size.width {
                        ctx.fill(Path(ellipseIn: CGRect(x: x - 1, y: y - 1, width: 2, height: 2)), with: .color(Color.white.opacity(0.06)))
                        x += step
                    }
                    y += step
                }
            }
            VStack(spacing: 14) {
                Icon("television-simple", size: 34).foregroundStyle(Theme.accentLight)
                Text("L'image est sur l'écran externe").font(Theme.sans(19, .semibold)).foregroundStyle(Theme.text)
                Text(deviceIsPad
                    ? "Tout l'iPad est le pavé tactile : un doigt déplace le pointeur, toucher clique, deux doigts pour le clic droit et la molette. Le trackpad et le clavier vont aussi à l'ordinateur."
                    : "Tout l'iPhone est le pavé tactile : un doigt déplace le pointeur, toucher clique, deux doigts pour le clic droit et la molette.")
                    .font(Theme.sans(14))
                    .foregroundStyle(Theme.muted)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 460)
            }
            .padding(24)
        }
        .ignoresSafeArea()
        .accessibilityElement(children: .combine)
    }
}

// MARK: - Capsules

struct StateCapsule<Actions: View>: View {
    var icon: String?
    var tint: Color = Theme.muted
    var neon = false
    let title: String
    let detail: String?
    @ViewBuilder var actions: () -> Actions

    var body: some View {
        HStack(spacing: 12) {
            if neon {
                Mark(size: 24, level: .lit)
            } else if let icon {
                Icon(icon, size: 20).foregroundStyle(tint)
            }
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                if let detail { Text(detail).font(Theme.sans(12.5)).foregroundStyle(Theme.muted) }
            }
            Spacer(minLength: 0)
            actions()
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .glass(16, tint: 0.55)
        .transition(.opacity.combined(with: .offset(y: -10)))
    }
}

// MARK: - Touches

/// Usages HID (page 7) des touches de la rangée et du menu Touches.
enum HID {
    static let escape: UInt16 = 0x29, tab: UInt16 = 0x2B, enter: UInt16 = 0x28, delete: UInt16 = 0x4C
    static let control: UInt16 = 0xE0, shift: UInt16 = 0xE1, alt: UInt16 = 0xE2, gui: UInt16 = 0xE3
    static let right: UInt16 = 0x4F, left: UInt16 = 0x50, down: UInt16 = 0x51, up: UInt16 = 0x52
    static let printScreen: UInt16 = 0x46, d: UInt16 = 0x07, l: UInt16 = 0x0F
    static func f(_ n: Int) -> UInt16 { UInt16(0x3A + n - 1) }
}

/// Le menu Touches : les raccourcis que le clavier de l'appareil ne fait pas.
struct KeysMenu: View {
    @ObservedObject var session: SessionController
    @Environment(\.dismiss) private var dismiss

    private var shortcuts: [(String, String, [UInt16], UInt16)] {
        [
            ("Ctrl + Alt + Suppr", String(localized: "Écran de sécurité"), [HID.control, HID.alt], HID.delete),
            ("Alt + Tab", String(localized: "Changer de fenêtre"), [HID.alt], HID.tab),
            ("⊞ + D", String(localized: "Afficher le bureau"), [HID.gui], HID.d),
            (String(localized: "Impr. écran"), String(localized: "Capture"), [], HID.printScreen),
            ("Alt + F4", String(localized: "Fermer la fenêtre"), [HID.alt], HID.f(4)),
            ("⊞ + L", String(localized: "Verrouiller l'ordinateur"), [HID.gui], HID.l),
            ("Ctrl + Maj + Échap", String(localized: "Gestionnaire des tâches"), [HID.control, HID.shift], HID.escape),
            ("F11", String(localized: "Plein écran"), [], HID.f(11)),
        ]
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Touches").font(Theme.sans(20, .semibold)).foregroundStyle(Theme.text)
                Text("Ce que le clavier de l'appareil ne sait pas faire").font(Theme.sans(13)).foregroundStyle(Theme.muted)
            }
            ScrollView {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 180), spacing: 10)], spacing: 10) {
                    ForEach(shortcuts, id: \.0) { s in
                        Button {
                            session.chord(s.2, s.3)
                            dismiss()
                        } label: {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(s.0).font(Theme.mono(13, .medium)).foregroundStyle(Theme.text)
                                Text(s.1).font(Theme.sans(12)).foregroundStyle(Theme.muted)
                            }
                            .frame(maxWidth: .infinity, minHeight: 56, alignment: .leading)
                            .padding(.horizontal, 14)
                            .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.surface))
                            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Theme.line))
                        }
                    }
                }
            }
        }
        .padding(20)
        .environment(\.colorScheme, .dark)
    }
}

/// La rangée au-dessus du clavier de l'écran (vue accessoire) : Échap, Tab,
/// Ctrl, Alt et ⊞ collants, flèches, F1 à F12, Ctrl + Alt + Suppr.
@MainActor
final class KeyboardRow {
    let view: UIView

    init(session: SessionController) {
        let host = UIHostingController(rootView: KeyboardRowView(session: session))
        host.view.backgroundColor = .clear
        host.view.frame = CGRect(x: 0, y: 0, width: 400, height: 52)
        host.view.autoresizingMask = [.flexibleWidth]
        host.sizingOptions = []
        view = host.view
        // La vue garde son contrôleur en vie.
        objc_setAssociatedObject(view, &KeyboardRow.key, host, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }

    nonisolated(unsafe) private static var key = 0
}

private struct KeyboardRowView: View {
    @ObservedObject var session: SessionController

    var body: some View {
        VStack(spacing: 2) {
            if !session.sticky.isEmpty {
                Text("\(stickyNames) tenue · la prochaine touche part avec elle")
                    .font(Theme.sans(11)).foregroundStyle(Theme.accentLight)
            }
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    key(String(localized: "Échap")) { session.tap(HID.escape) }
                    key("Tab") { session.tap(HID.tab) }
                    sticky("Ctrl", HID.control)
                    sticky("Alt", HID.alt)
                    sticky("⊞", HID.gui)
                    key("←") { session.tap(HID.left) }
                    key("↑") { session.tap(HID.up) }
                    key("↓") { session.tap(HID.down) }
                    key("→") { session.tap(HID.right) }
                    ForEach(1...12, id: \.self) { n in
                        key("F\(n)") { session.tap(HID.f(n)) }
                    }
                    key("Ctrl+Alt+Suppr") { session.chord([HID.control, HID.alt], HID.delete) }
                    key(String(localized: "Touches")) { session.keysMenu = true }
                }
                .padding(.horizontal, 8)
            }
        }
        .frame(height: session.sticky.isEmpty ? 44 : 60)
        .background(Theme.bar.opacity(0.92))
    }

    private var stickyNames: String {
        [(HID.control, "Ctrl"), (HID.alt, "Alt"), (HID.gui, "⊞")].filter { session.sticky.contains($0.0) }.map(\.1).joined(separator: " + ")
    }

    private func key(_ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(label)
                .font(Theme.sans(13, .medium))
                .foregroundStyle(Theme.text)
                .padding(.horizontal, 10)
                .frame(minWidth: 40, minHeight: 34)
                .background(RoundedRectangle(cornerRadius: 7).fill(Theme.surface2))
        }
    }

    private func sticky(_ label: String, _ usage: UInt16) -> some View {
        let on = session.sticky.contains(usage)
        return Button { session.toggleSticky(usage) } label: {
            Text(label)
                .font(Theme.sans(13, .semibold))
                .foregroundStyle(on ? Color.white : Theme.text)
                .padding(.horizontal, 10)
                .frame(minWidth: 40, minHeight: 34)
                .background(RoundedRectangle(cornerRadius: 7).fill(on ? Theme.accentFill : Theme.surface2))
        }
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

// MARK: - Échec

/// « La connexion a échoué » (avant l'image) ou « La session s'est arrêtée »,
/// puis la raison en une phrase.
struct SessionFailure: View {
    let host: Host
    /// Le titre : le nom de l'ordinateur, ou « Invité sur {nom} ».
    var title: String?
    let message: String
    /// Avant la première image.
    var connecting = true
    /// Fin d'une session d'invité par le partage : la raison seule, sans titre d'échec.
    var shareEnd = false
    let done: () -> Void

    private var reason: String {
        let text = message.prefix(1).uppercased() + message.dropFirst()
        return text.hasSuffix(".") ? text : text + "."
    }

    var body: some View {
        ZStack(alignment: .topLeading) {
            HostBackdrop(host: host, online: false)
            VStack(alignment: .leading, spacing: 16) {
                if !shareEnd {
                    HStack(spacing: 10) {
                        Icon("warning", size: 20).foregroundStyle(Theme.warn)
                        Theme.eyebrow(connecting ? String(localized: "La connexion a échoué") : String(localized: "La session s'est arrêtée"), color: Theme.warnText)
                    }
                }
                Text(title ?? host.name).font(Theme.display(40, 680)).tracking(-40 * 0.04).foregroundStyle(Theme.text)
                Text(reason)
                    .font(Theme.sans(16)).foregroundStyle(Theme.text2)
                Button("Retour", action: done)
                    .buttonStyle(PrimaryButton())
                    .frame(maxWidth: 320)
                    .padding(.top, 8)
            }
            .frame(maxWidth: 520, alignment: .leading)
            .padding(32)
        }
        .ignoresSafeArea()
    }
}

// MARK: - Nombres

enum Format {
    private static var french: Bool { LanguageChoice.effective == "fr" }

    /// « 9,8 ms » (une décimale sous 100 ms).
    static func ms(_ v: Double) -> String {
        let s = v >= 100 ? String(Int(v.rounded())) : String(format: "%.1f", v)
        return (french ? s.replacingOccurrences(of: ".", with: ",") : s) + " ms"
    }
}
