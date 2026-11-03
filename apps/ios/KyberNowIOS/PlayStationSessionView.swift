// Une partie sur console PlayStation à l'écran, sur iPhone (toujours en
// paysage) et sur iPad (docs/link-ui.md, « Consoles PlayStation ») : l'image
// plein cadre, la languette et la barre de la partie (même présentation que
// celle d'une session d'ordinateur), les statistiques (feuille sur iPhone,
// panneau sur iPad), la manette à l'écran aux signes PlayStation, le champ
// du clavier de la console, et ce qui se dit par-dessus l'image (connexion,
// contenu protégé, fin).

import GameController
import KyberKit
import SwiftUI

struct PlayStationSessionView: View {
    @ObservedObject var session: PlayStationSession

    private var streaming: Bool { session.phase == .streaming }

    var body: some View {
        GeometryReader { g in
            ZStack {
                Color.black
                // Aucun effet SwiftUI sur la vue Metal : il la figerait.
                StreamStage(view: session.streamView)
                    .ignoresSafeArea()

                if streaming, session.screenPad {
                    ScreenPadOverlay(layout: session.screenPadLayout, size: .medium, opacity: 0.6,
                                     haptics: PadHaptics.available ? .medium : .off, playstation: true) { session.toggleScreenPadLayout() }
                        .ignoresSafeArea()
                        .transition(.opacity)
                }

                if streaming {
                    chrome(size: g.size)
                }

                PlayStationOverlay(session: session)
            }
        }
        // Pas la zone du clavier : le champ du clavier de la console monte au-dessus.
        .ignoresSafeArea(.container)
        .statusBarHidden()
        .persistentSystemOverlays(.hidden)
        .defersSystemGestures(on: .all)
        .sheet(isPresented: Binding(get: { session.statsShown && !DeviceDisplay.isPad }, set: { session.statsShown = $0 })) {
            PlayStationStatsView(session: session)
                .presentationDetents([.medium])
                .presentationBackgroundInteraction(.enabled(upThrough: .medium))
                .presentationBackground(Theme.bar)
        }
    }

    @ViewBuilder
    private func chrome(size: CGSize) -> some View {
        ZStack {
            if session.cantDisplay {
                StateCapsule(icon: "warning", tint: Theme.warn, title: PSText.cantDisplay, detail: nil) { EmptyView() }
                    .frame(maxWidth: 620)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, session.barOpen ? 80 : 44)
                    .padding(.horizontal, 16)
            }

            if session.barOpen {
                PlayStationBar(session: session)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .padding(.top, 10)
                    .padding(.horizontal, 16)
                    .transition(.move(edge: .top).combined(with: .opacity))
            } else {
                PlayStationTab(session: session)
                    .frame(maxHeight: .infinity, alignment: .top)
                    .transition(.opacity)
            }

            if session.consoleKeyboard {
                ConsoleKeyboardField(session: session)
                    .frame(maxWidth: 560)
                    .frame(maxHeight: .infinity, alignment: .bottom)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 16)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }

            // iPad : le panneau des statistiques, en bas à droite.
            if DeviceDisplay.isPad, session.statsShown {
                PlayStationStatsView(session: session)
                    .frame(width: min(520, size.width - 48))
                    .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Theme.bar))
                    .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Theme.line))
                    .overlay(alignment: .topTrailing) {
                        Button { session.statsShown = false } label: {
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

// MARK: - Languette et barre

/// La languette en haut de l'image : elle ouvre la barre ; « Par le relais »
/// quand la partie passe par une Station.
private struct PlayStationTab: View {
    @ObservedObject var session: PlayStationSession

    var body: some View {
        let relay = session.path.viaRelay
        HStack(spacing: 0) {
            if relay {
                HStack(spacing: 5) {
                    Icon("globe-hemisphere-west", size: 12, bold: true).foregroundStyle(Theme.accentLight)
                    Text(LinkText.viaRelay).font(Theme.sans(11.5, .semibold)).foregroundStyle(Theme.text).lineLimit(1).fixedSize()
                }
                .padding(.horizontal, 8)
                .frame(minHeight: 26)
                .accessibilityElement(children: .combine)
            }
            Button { session.setBar(true) } label: {
                Image(systemName: "chevron.down").font(.system(size: 11, weight: .bold)).foregroundStyle(Theme.muted)
                    .frame(width: 88, height: 26)
                    .contentShape(Rectangle())
            }
            .accessibilityLabel(Text("Afficher la barre de session"))
        }
        .padding(.horizontal, relay ? 6 : 0)
        .background(UnevenRoundedRectangle(bottomLeadingRadius: 14, bottomTrailingRadius: 14).fill(.ultraThinMaterial))
        .background(UnevenRoundedRectangle(bottomLeadingRadius: 14, bottomTrailingRadius: 14).fill(Theme.background.opacity(0.5)))
        .environment(\.colorScheme, .dark)
    }
}

/// La barre de la partie : une ligne, tout au pouce ; la manette la parcourt
/// (Options + Create + L1 + R1).
struct PlayStationBar: View {
    @ObservedObject var session: PlayStationSession

    var body: some View {
        HStack(spacing: 0) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 4) {
                    Mark(size: 20, level: .lit).padding(.leading, 6)
                    if DeviceDisplay.isPad {
                        Text(session.console.name).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text).lineLimit(1).padding(.horizontal, 6)
                        separator
                    }
                    button(.stats) { label(icon: "monitor", String(localized: "Statistiques")) }
                    separator
                    button(.sound) { label(icon: session.muted ? "speaker-slash" : "speaker-high", session.muted ? PSText.unmute : PSText.mute) }
                    button(.screenPad) { label(icon: "game-controller", ScreenPadText.name, on: session.screenPad) }
                    button(.settings) { label(icon: "gear-six", String(localized: "Réglages")) }
                        .accessibilityLabel(Text(LinkText.settingsTitle(session.console.name)))
                    button(.rest) { label(system: "moon.zzz", PSText.rest) }
                }
                .padding(.leading, 6)
            }
            HStack(spacing: 4) {
                separator
                if session.path.viaRelay { label(icon: "globe-hemisphere-west", LinkText.viaRelay) }
                Button { session.setBar(false) } label: {
                    Image(systemName: "chevron.up").font(.system(size: 13, weight: .bold)).foregroundStyle(Theme.text2)
                        .frame(width: 44, height: 44)
                }
                .accessibilityLabel(Text("Masquer la barre"))
                button(.disconnect) {
                    Icon("power", size: 18)
                        .foregroundStyle(Theme.dangerText)
                        .frame(minWidth: 44, minHeight: 44)
                        .background(RoundedRectangle(cornerRadius: 10).fill(Theme.danger.opacity(0.14)))
                        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Theme.danger.opacity(0.4)))
                }
                .accessibilityLabel(Text("Se déconnecter"))
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

    /// Une commande, cerclée quand la manette est dessus.
    private func button<L: View>(_ item: PlayStationSession.BarItem, @ViewBuilder label: () -> L) -> some View {
        let focused = session.padMode && session.barItems.firstIndex(of: item) == session.padFocus
        return Button { session.activate(item) } label: {
            label()
                .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Theme.accentLight, lineWidth: focused ? 2 : 0))
        }
    }

    private func label(icon: String? = nil, system: String? = nil, _ text: String, on: Bool = false) -> some View {
        VStack(spacing: 2) {
            if let icon { Icon(icon, size: 18).foregroundStyle(on ? Theme.text : Theme.accentLight) }
            if let system { Image(systemName: system).font(.system(size: 16, weight: .regular)).frame(height: 18).foregroundStyle(Theme.accentLight) }
            Text(text).font(Theme.sans(9.5)).foregroundStyle(Theme.text2).lineLimit(1)
        }
        .frame(minWidth: 48, minHeight: 44)
        .padding(.horizontal, 4)
        .background(RoundedRectangle(cornerRadius: 10).fill(on ? Theme.accentFill.opacity(0.35) : .clear))
    }

    private var separator: some View {
        Rectangle().fill(Color.white.opacity(0.12)).frame(width: 1, height: 28)
    }
}

// MARK: - Statistiques

/// Les mesures de la partie, une fois par seconde : ce qui est demandé, ce
/// qui arrive, les pertes depuis le début, le chemin, l'agrandissement.
struct PlayStationStatsView: View {
    @ObservedObject var session: PlayStationSession

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Statistiques").font(Theme.sans(17, .semibold)).foregroundStyle(Theme.text)
            if let s = session.stats {
                line(PSText.statsAsked(s, fps: session.requestedFps))
                line(PSText.statsReceived(s))
            } else {
                line("—")
            }
            line(PSText.statsLosses(lost: session.lostTotal, repaired: session.repairedTotal))
            if session.path.viaRelay { line("\(LinkText.path) : \(session.path.label)") }
            if let u = session.upscale, case let .active(w, h) = u.state {
                line("MetalFX · \(w) × \(h)" + (u.gpuMs.map { " · " + Format.ms($0) } ?? ""))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(20)
        .environment(\.colorScheme, .dark)
        .accessibilityElement(children: .combine)
    }

    private func line(_ text: String) -> some View {
        Text(text).font(Theme.mono(13)).foregroundStyle(Theme.text2).lineLimit(2)
    }
}

// MARK: - Clavier de la console

/// La console a ouvert son clavier : le texte s'écrit ici (clavier du
/// système sur iPhone, clavier branché sur iPad) et part à chaque changement ;
/// Entrée (« OK ») valide, « Annuler » (ou Échap) refuse.
private struct ConsoleKeyboardField: View {
    @ObservedObject var session: PlayStationSession
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(GCKeyboard.coalesced != nil ? PSText.consoleKeyboard : PSText.typeYourText)
                .font(Theme.sans(13, .medium)).foregroundStyle(Theme.text2)
            HStack(spacing: 10) {
                TextField(text: $session.consoleText) { Text(verbatim: PSText.typeYourText) }
                    .font(Theme.sans(16))
                    .foregroundStyle(Theme.text)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .submitLabel(.done)
                    .focused($focused)
                    .onSubmit { session.consoleKeyboardAccept() }
                    .onChange(of: session.consoleText) { session.consoleTextChanged() }
                    .onKeyPress(.escape) {
                        session.consoleKeyboardReject()
                        return .handled
                    }
                    .padding(.horizontal, 12)
                    .frame(height: 44)
                    .background(RoundedRectangle(cornerRadius: 10).fill(Color.white.opacity(0.08)))
                Button(PSText.cancel) { session.consoleKeyboardReject() }
                    .buttonStyle(GlassButton(height: 44, fontSize: 14))
            }
        }
        .padding(14)
        .glass(16, tint: 0.6)
        .onAppear { focused = true }
    }
}

// MARK: - Par-dessus l'image

/// La connexion, la fin (et sa raison), « Fermer ».
private struct PlayStationOverlay: View {
    @ObservedObject var session: PlayStationSession

    var body: some View {
        switch session.phase {
        case .connecting:
            ZStack {
                Theme.background
                HStack(spacing: 12) {
                    ProgressView().tint(Theme.text2)
                    Text(PSText.connecting).font(Theme.sans(16)).foregroundStyle(Theme.text2)
                }
            }
            .ignoresSafeArea()
            .overlay(alignment: .bottom) {
                Button(PSText.cancel) { session.close() }
                    .buttonStyle(GlassButton(height: 44, fontSize: 14))
                    .padding(.bottom, 32)
            }
        case .streaming:
            EmptyView()
        case let .ended(text):
            ZStack {
                Theme.background
                VStack(spacing: 18) {
                    Text(session.console.name).font(Theme.sans(20, .semibold)).foregroundStyle(Theme.text)
                    if let text {
                        Text(text).font(Theme.sans(16)).foregroundStyle(Theme.text2)
                            .multilineTextAlignment(.center)
                            .frame(maxWidth: 480)
                    }
                    Button(String(localized: "Fermer")) { session.close() }
                        .buttonStyle(GlassButton(height: 44, fontSize: 15))
                }
                .padding(24)
            }
            .ignoresSafeArea()
        }
    }
}
