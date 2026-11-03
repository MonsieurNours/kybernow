// Barre de la partie sur console PlayStation (docs/link-ui.md, « Consoles
// PlayStation », « Barre de la partie ») : celle d'une session d'ordinateur
// (mêmes pièces : SessionChrome.swift), avec ce qui a du sens pour une
// console : le nom, les statistiques, le son, la souris, les réglages, le
// plein écran, la mise en veille et « Se déconnecter ». Et son panneau des
// mesures, en bas à droite.

import AppKit
import KyberKit
import SwiftUI

/// Couche SwiftUI posée sur l'image : elle laisse passer clics et mouvements
/// vers la vue du clavier et de la souris, sauf sur la barre ouverte et sur le
/// bouton de fermeture du panneau des mesures.
final class PlayStationChromeView: NSHostingView<PlayStationChrome> {
    weak var session: PlayStationSession?

    override func hitTest(_ point: NSPoint) -> NSView? {
        guard let session else { return nil }
        let local = convert(point, from: superview)
        let fromTop = isFlipped ? local.y : bounds.height - local.y
        if session.statsPanel, session.statsCloseRect.insetBy(dx: -4, dy: -4).contains(CGPoint(x: local.x, y: fromTop)) {
            return super.hitTest(point)
        }
        guard session.barVisible else { return nil }
        return fromTop < session.barHeight ? super.hitTest(point) : nil
    }
}

struct PlayStationChrome: View {
    @ObservedObject var session: PlayStationSession

    var body: some View {
        ZStack(alignment: .top) {
            // Le trait de repère du bord haut est tracé dans l'image (VideoRenderer).
            if session.barVisible {
                PlayStationBar(session: session)
                    .padding(.top, 14)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
            if session.phase == .streaming, session.statsPanel {
                PlayStationStatsPanel(session: session)
                    .padding(20)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
                    .transition(.opacity.combined(with: .offset(x: 24)))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .onPreferenceChange(StatsCloseRectKey.self) { rect in session.statsCloseRect = rect }
        .onPreferenceChange(SessionBarRectKey.self) { rect in
            // Sans barre, la valeur retombe à zéro : la dernière mesure reste.
            if rect != .zero { session.barRect = rect }
        }
        .animation(Theme.easeOut, value: session.barVisible)
        .animation(Theme.easeOut, value: session.statsPanel)
        .preferredColorScheme(.dark)
    }
}

private struct PlayStationBar: View {
    @ObservedObject var session: PlayStationSession

    private func ring(_ item: PlayStationSession.BarItem) -> some View {
        let index = session.barItems.firstIndex(of: item) ?? -1
        return BarRing(on: session.padMode && session.padFocus == index, focus: session.padFocus)
    }

    var body: some View {
        VStack(spacing: 10) {
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
                PadBanner(away: PSText.padAway, accept: "✕", back: "○", combo: "L1 R1 OPTIONS CREATE", comboLabel: PSText.padBack)
                    .transition(.opacity.combined(with: .offset(y: -6)))
            }
        }
    }

    private func row(compact: Bool) -> some View {
        HStack(spacing: 6) {
            Mark(size: 22, level: .lit)
            Text(session.console.name).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                .padding(.leading, 6).padding(.trailing, 8)
            BarDivider()
            statsButton(compact: compact)
                .overlay(ring(.stats))
            BarDivider()
            Button { session.toggleMute() } label: {
                BarChip(icon: session.muted ? "speaker-slash" : "speaker-high", text: session.muted ? PSText.unmute : PSText.mute, compact: compact)
            }
            .buttonStyle(.plain)
            .help(session.muted ? PSText.unmute : PSText.mute)
            .overlay(ring(.sound))
            // Une manette branchée l'emporte sur le clavier et la souris : pas de souris.
            if !session.padConnected {
                Button { session.toggleMouse() } label: {
                    BarChip(icon: session.mouseCaptured ? "crosshair" : "cursor",
                            text: session.mouseCaptured ? PSText.giveMouse : PSText.takeMouse, compact: compact)
                }
                .buttonStyle(.plain)
                .help((session.mouseCaptured ? PSText.giveMouse : PSText.takeMouse) + " (⌃⌥⌘M)")
                .overlay(ring(.mouse))
            }
            Button { session.openConsoleSettings() } label: {
                BarChip(icon: "gear-six", text: LinkText.settingsTitle(session.console.name), compact: compact)
            }
            .buttonStyle(.plain)
            .help(LinkText.settingsTitle(session.console.name))
            .overlay(ring(.settings))
            BarDivider()
            Button { session.toggleFullScreen() } label: {
                Icon(session.isFullScreen ? "corners-in" : "corners-out", size: 16, bold: true)
                    .foregroundStyle(Theme.text2)
                    .frame(width: 38, height: 34)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help((session.isFullScreen ? PSText.exitFullScreen : String(localized: "Plein écran")) + " (⌃⌥⌘F)")
            .accessibilityLabel(session.isFullScreen ? PSText.exitFullScreen : String(localized: "Plein écran"))
            .overlay(ring(.fullScreen))
            Button { session.goToRest() } label: {
                BarChip(icon: "television-simple", text: PSText.rest, compact: compact)
            }
            .buttonStyle(.plain)
            .help(PSText.rest)
            .overlay(ring(.rest))
            BarEndButton(title: String(localized: "Se déconnecter")) { session.close() }
                .help("Se déconnecter (⌃⌥⌘D)")
                .overlay(ring(.disconnect))
        }
        .padding(.leading, 14)
        .padding(.trailing, 8)
        .fixedSize()
    }

    /// Les images reçues par seconde (et le format), qui ouvrent le panneau des mesures.
    private func statsButton(compact: Bool) -> some View {
        Button { session.toggleStatsPanel() } label: {
            HStack(spacing: 10) {
                if let s = session.stats {
                    Text(PSText.fps(Int(s.fps.rounded()))).font(Theme.mono(13)).foregroundStyle(Theme.text2)
                    if !compact, s.width > 0 {
                        Text(verbatim: "\(s.width) × \(s.height) · \(s.hevc ? "H.265" : "H.264")\(s.hdr ? " · HDR" : "")")
                            .font(Theme.mono(11)).foregroundStyle(Theme.muted)
                    }
                } else {
                    Icon("chart-line", size: 16, bold: true).foregroundStyle(Theme.text2)
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
}

/// Les mesures de la partie, une fois par seconde : ce qui est demandé à la
/// console, ce qui arrive, et les images perdues et réparées depuis le début.
/// Comme celui d'une session d'ordinateur, il laisse tout passer sauf son
/// bouton de fermeture.
private struct PlayStationStatsPanel: View {
    @ObservedObject var session: PlayStationSession

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 12) {
                Theme.eyebrow(String(localized: "Statistiques"))
                Spacer(minLength: 0)
                Keycap("⌃⌥⌘S").opacity(0.8)
                Button { session.toggleStatsPanel() } label: {
                    Icon("x", size: 13, bold: true)
                        .foregroundStyle(Theme.text2)
                        .frame(width: 30, height: 30)
                        .background(Color.white.opacity(0.07), in: Circle())
                        .contentShape(Circle())
                }
                .buttonStyle(.plain)
                .help("Fermer les statistiques (⌃⌥⌘S)")
                .accessibilityLabel("Fermer les statistiques")
                .background(GeometryReader { g in
                    Color.clear.preference(key: StatsCloseRectKey.self, value: g.frame(in: SessionChrome.space))
                })
            }
            if let s = session.stats, s.width > 0 {
                VStack(alignment: .leading, spacing: 8) {
                    line(PSText.statsAsked(s, fps: session.requestedFps))
                    line(PSText.statsReceived(s))
                    line(PSText.statsLosses(lost: session.lostTotal, repaired: session.repairedTotal))
                }
            } else {
                Text("Les statistiques s'affichent dès que l'image arrive.")
                    .font(Theme.sans(12.5))
                    .foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(18)
        .frame(width: 392)
        // Fond opaque, sans verre dépoli (recalculé à chaque image du flux).
        .background {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(Theme.background.opacity(0.94))
                .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Color.white.opacity(0.09)))
        }
        .environment(\.colorScheme, .dark)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Statistiques")
    }

    private func line(_ text: String) -> some View {
        Text(text).font(Theme.mono(12.5)).foregroundStyle(Theme.text2)
            .contentTransition(.numericText())
            .fixedSize(horizontal: false, vertical: true)
    }
}
