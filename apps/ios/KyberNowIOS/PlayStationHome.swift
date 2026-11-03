// Consoles PlayStation à l'accueil de l'iPhone et de l'iPad (textes :
// docs/link-ui.md, « Consoles PlayStation ») : la console en scène (état,
// nom, modèle, jeu en cours, bouton principal, puce du compte, puce « HDR »,
// puce « Auto » / « Personnalisé », engrenage), sa carte dans la rangée des
// ordinateurs, et la carte « Sur le réseau » d'une console pas encore
// associée. Même composition que l'ordinateur en scène (HomeView) ; le
// modèle est celui du Mac (`PlayStationStore`, PlayStation.swift).

import KyberKit
import SwiftUI

// MARK: - La console en scène

/// État, nom, modèle et jeu en cours, dans l'ordre de link-ui.
struct ConsoleStageHero: View {
    let console: PlayStationConsole
    @ObservedObject var store: PlayStationStore
    let wide: Bool
    let revealed: Bool

    var body: some View {
        let status = store.status(console)
        let gap: CGFloat = wide ? 16 : 11
        VStack(alignment: .leading, spacing: gap) {
            HomeChip(height: wide ? 32 : 30, fontSize: wide ? 14 : 13) {
                HomeStatusDot(online: status.online, size: wide ? 8 : 7)
                Text(status.text).lineLimit(1)
            }
            .id(status)
            .homeRise(revealed, 0.1)
            Text(verbatim: console.name)
                .font(Theme.display(wide ? 96 : 54, 680))
                .tracking(wide ? -96 * 0.045 : -54 * 0.04)
                .lineSpacing(wide ? -14 : -8)
                .lineLimit(2)
                .minimumScaleFactor(0.5)
                .foregroundStyle(Theme.text)
                .frame(minHeight: wide ? 172 : 100, alignment: .bottomLeading)
                .homeRise(revealed, 0.18)
                .accessibilityAddTraits(.isHeader)
            Text(verbatim: console.model)
                .font(Theme.sans(wide ? 17 : 14.5))
                .foregroundStyle(Theme.text2)
                .lineLimit(1)
                .homeRise(revealed, 0.24)
            if let game = store.live(console)?.runningApp {
                HStack(alignment: .firstTextBaseline, spacing: wide ? 10 : 9) {
                    Icon("game-controller", size: wide ? 19 : 17).foregroundStyle(Theme.accent)
                        .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 3 }
                    Text(verbatim: game)
                        .font(Theme.sans(wide ? 16 : 13.5))
                        .foregroundStyle(Theme.text2)
                        .lineLimit(2)
                }
                .homeRise(revealed, 0.32)
            }
        }
        .animation(Theme.easeOut, value: status)
    }
}

/// Le bouton principal, puis la puce du compte, la puce « HDR », « Auto » ou
/// « Personnalisé » et l'engrenage des réglages.
struct ConsoleStageActions: View {
    @ObservedObject var app: AppModel
    let console: PlayStationConsole
    @ObservedObject var store: PlayStationStore
    let wide: Bool
    let glow: NeonLevel

    private var status: PlayStationStore.Status { store.status(console) }
    private var paired: Bool { store.paired.contains(console.id) }

    var body: some View {
        if wide {
            HStack(spacing: 16) {
                cta
                chips
                settingsButton
            }
        } else {
            VStack(alignment: .leading, spacing: 10) {
                cta
                // Trois puces et l'engrenage n'entrent pas toujours sur un iPhone :
                // la puce du compte passe alors au-dessus.
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: 8) {
                        chips
                        Spacer(minLength: 0)
                        settingsButton
                    }
                    VStack(alignment: .leading, spacing: 10) {
                        accountChip
                        HStack(spacing: 8) {
                            hdrChip
                            autoChip
                            Spacer(minLength: 0)
                            settingsButton
                        }
                    }
                }
            }
        }
    }

    // MARK: Bouton principal

    private enum Main {
        case pair, play, playAway, wake, waking, reaching, retry, searching
    }

    private var main: Main {
        if !paired { return .pair }
        if store.waking.contains(console.id) { return .waking }
        if store.reaching.contains(console.id) { return .reaching }
        switch status {
        case .away: return .playAway
        case .on: return .play
        case .standby: return .wake
        case .offline: return .retry
        case .searching: return .searching
        }
    }

    private var cta: some View {
        let m = main
        let lit = m != .retry && m != .searching
        let busy = m == .waking || m == .reaching
        return Button { act(m) } label: {
            HStack(spacing: 12) {
                Text(title(m))
                if !wide { Spacer(minLength: 0) }
                if busy {
                    HomeSpinner(size: 20)
                } else {
                    Icon(icon(m), size: 20, bold: true)
                }
            }
            .padding(.leading, wide ? 22 : 20)
            .padding(.trailing, wide ? 26 : 18)
            .frame(maxWidth: wide ? nil : .infinity)
        }
        .buttonStyle(HomeCTAStyle(online: lit || busy, height: wide ? 58 : 56, fontSize: wide ? 18 : 17, glow: lit ? glow : .off))
        .keyboardShortcut(.defaultAction)
        .hoverEffect(.highlight)
        .disabled(busy || m == .searching)
    }

    private func title(_ m: Main) -> String {
        switch m {
        case .pair: return PSText.pair
        case .play, .playAway, .searching: return PSText.play
        case .wake: return PSText.wakeAndPlay
        case .waking: return PSText.waking
        case .reaching: return PSText.connecting
        case .retry: return String(localized: "Réessayer")
        }
    }

    private func icon(_ m: Main) -> String {
        switch m {
        case .pair: return "link-simple"
        case .wake: return "power"
        case .retry: return "arrows-clockwise"
        default: return "arrow-right"
        }
    }

    private func act(_ m: Main) {
        switch m {
        case .pair: store.pairing = PairTarget(console: console, live: store.live(console))
        case .play: store.play(console, app: app)
        case .playAway: store.playAway(console, app: app)
        case .wake: store.wakeAndPlay(console, app: app)
        case .retry: Task { await store.searchOnce() }
        case .waking, .reaching, .searching: break
        }
    }

    // MARK: Puces

    @ViewBuilder private var chips: some View {
        accountChip
        hdrChip
        autoChip
    }

    /// Associée avec plusieurs comptes : la puce « {pseudo} » ouvre le choix
    /// du compte (le profil avec lequel la console ouvre la partie).
    @ViewBuilder private var accountChip: some View {
        if paired, store.pairedAccounts(console).count > 1, let chosen = store.chosenAccount(console) {
            Menu {
                ForEach(store.pairedAccounts(console)) { a in
                    Button { store.choose(a, for: console) } label: {
                        if a.id == chosen.id {
                            Label { Text(verbatim: store.name(a)) } icon: { Image(systemName: "checkmark") }
                        } else {
                            Text(verbatim: store.name(a))
                        }
                    }
                }
            } label: {
                HStack(spacing: 8) {
                    Icon("user", size: 16, bold: true)
                    Text(verbatim: store.name(chosen)).lineLimit(1)
                    Image(systemName: "chevron.down").font(.system(size: 11, weight: .bold))
                }
                .font(Theme.sans(wide ? 15 : 14, .semibold))
                .foregroundStyle(Theme.text)
                .padding(.horizontal, 16)
                .frame(minHeight: 44)
                .padding(4)
                .glass(12)
                .contentShape(Rectangle())
            }
            .accessibilityLabel(Text(verbatim: store.name(chosen)))
        }
    }

    /// La puce « HDR » : PS5, écran HDR ; mêmes règles que pour un ordinateur.
    @ViewBuilder private var hdrChip: some View {
        if paired, console.hasHDR, HDRDisplay.isHDR(nil) {
            HomeHDRChip(state: HDRState(shown: true, available: true, reason: nil), on: console.hdr, height: 44, fontSize: wide ? 15 : 14) {
                store.setHDR(!console.hdr, for: console)
            }
        }
    }

    /// « Auto » ou « Personnalisé » (pas de profils pour une console) : ouvre les réglages.
    private var autoChip: some View {
        Button { app.openConsoleSettings(console) } label: {
            HStack(spacing: 8) {
                Icon("sliders-horizontal", size: 16, bold: true)
                Text(store.isCustom(console) ? PSText.custom : PSText.auto).lineLimit(1)
            }
            .font(Theme.sans(wide ? 15 : 14, .semibold))
            .foregroundStyle(Theme.text2)
            .padding(.horizontal, 16)
            .frame(minHeight: 44)
            .padding(4)
            .glass(12)
            .contentShape(Rectangle())
        }
        .buttonStyle(HomePressStyle())
        .accessibilityHint(Text(LinkText.settingsTitle(console.name)))
    }

    private var settingsButton: some View {
        Button { app.openConsoleSettings(console) } label: {
            if wide {
                HStack(spacing: 9) {
                    Icon("gear-six", size: 20)
                    Text("Réglages")
                }
                .font(Theme.sans(15, .medium))
                .foregroundStyle(Theme.text)
                .padding(.leading, 14).padding(.trailing, 18)
                .frame(height: 52)
                .glass(12)
                .contentShape(Rectangle())
            } else {
                Icon("gear-six", size: 22)
                    .foregroundStyle(Theme.text)
                    .frame(width: 52, height: 52)
                    .glass(12)
                    .contentShape(Rectangle())
            }
        }
        .buttonStyle(HomePressStyle())
        .hoverEffect(.highlight)
        .accessibilityLabel(Text(LinkText.settingsTitle(console.name)))
    }
}

// MARK: - Dans la rangée

/// Une console de la rangée : sa signature (pas de dernière image), son nom
/// et son état ; celle qui est en scène est soulignée de Spectre.
struct HomeConsoleCard: View {
    let console: PlayStationConsole
    let status: PlayStationStore.Status
    let selected: Bool
    let m: HomeCardMetrics

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ZStack(alignment: .bottomLeading) {
                Signature(seed: console.hostID.isEmpty ? console.name : console.hostID)
                    .saturation(status.online == false ? 0 : 1)
                    .opacity(status.online == false ? 0.6 : 1)
                Text(verbatim: console.model)
                    .font(Theme.mono(m.detail - 1, .medium))
                    .tracking(0.6)
                    .foregroundStyle(Theme.text2)
                    .padding(.horizontal, m.padding)
                    .padding(.bottom, 6)
            }
            .frame(width: m.width, height: m.image)
            .clipped()
            VStack(alignment: .leading, spacing: 4) {
                Text(verbatim: console.name)
                    .font(Theme.sans(m.name, .semibold))
                    .foregroundStyle(status.online != false ? Theme.text : Theme.muted)
                    .lineLimit(1)
                    .minimumScaleFactor(0.75)
                HStack(spacing: 6) {
                    HomeStatusDot(online: status.online, size: 6, pulse: false)
                    Text(status.text).lineLimit(1)
                }
                .font(Theme.sans(m.detail))
                .foregroundStyle(Theme.muted)
            }
            .padding(.horizontal, m.padding)
            .padding(.vertical, m.padding - 3)
            Spacer(minLength: 0)
        }
        .frame(width: m.width, height: m.height, alignment: .topLeading)
        .overlay(alignment: .bottom) {
            if selected {
                Capsule()
                    .fill(Theme.accent)
                    .frame(height: 3)
                    .padding(.horizontal, m.padding)
                    .padding(.bottom, 6)
                    .shadow(color: Theme.accentFill, radius: 5)
                    .transition(.opacity)
            }
        }
        .glass(m.radius)
        .clipShape(RoundedRectangle(cornerRadius: m.radius, style: .continuous))
        .contentShape(.contextMenuPreview, RoundedRectangle(cornerRadius: m.radius, style: .continuous))
        .contentShape(.hoverEffect, RoundedRectangle(cornerRadius: m.radius, style: .continuous))
    }
}

#if DEBUG
/// Démonstration (simulateur) : `-consoleIndex N` met la console N en scène
/// (avec `-demo YES -demoPlayStation YES`), `-screen consoleSettings` ouvre
/// ses réglages (`-consoleSection picture|controls`).
enum ConsoleDemo {
    @MainActor
    static func apply(_ app: AppModel) {
        let d = UserDefaults.standard
        guard d.bool(forKey: "demoPlayStation") else { return }
        let list = app.playstation.consoles
        if d.object(forKey: "consoleIndex") != nil, list.indices.contains(d.integer(forKey: "consoleIndex")) {
            app.selected = list[d.integer(forKey: "consoleIndex")].id
        }
        if d.string(forKey: "screen") == "consoleSettings", let c = list.first(where: { $0.id == app.selected }) ?? list.first {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { app.openConsoleSettings(c) }
        }
    }
}
#endif
