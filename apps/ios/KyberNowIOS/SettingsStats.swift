// Rubrique « Statistiques » des réglages d'un ordinateur : la carte
// « Dernière session », sur le modèle de celle de l'app Mac (la médiane en
// grand, la règle des trames, le 95e centile, la durée et ce qui a été
// transmis), et la règle elle-même. Textes et chiffres communs : LinkUI.swift.

import KyberKit
import SwiftUI

/// La dernière session d'au moins 3 s, lue contre la durée d'une image.
struct LastSessionCard: View {
    let summary: LastSessionSummary?
    var wide = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline) {
                Theme.eyebrow(StatsText.lastSession)
                Spacer(minLength: 8)
                if let summary, !summary.when.isEmpty {
                    Text(verbatim: summary.when).font(Theme.sans(wide ? 13 : 12.5)).foregroundStyle(Theme.muted).lineLimit(1)
                }
            }
            if let summary {
                if let median = summary.median {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(verbatim: LinkFormat.ms(median)).font(Theme.mono(wide ? 34 : 30, .medium)).foregroundStyle(Theme.text)
                        Text(StatsText.median).font(Theme.mono(12.5)).foregroundStyle(Theme.muted)
                    }
                    FrameRuler(median: median, p95: summary.p95)
                    if let p95 = summary.p95 {
                        Text(StatsText.under(p95)).font(Theme.sans(12.5)).foregroundStyle(Theme.text2)
                    }
                }
                if !summary.transmitted.isEmpty {
                    Text(verbatim: summary.transmitted).font(Theme.sans(13).monospacedDigit()).foregroundStyle(Theme.muted)
                        .lineLimit(1)
                        .minimumScaleFactor(0.8)
                }
                // « Chemin : Direct » ou « Chemin : Par le relais » (rien pour une session d'avant le relais).
                if let path = summary.pathLine {
                    Text(verbatim: path).font(Theme.sans(13).monospacedDigit()).foregroundStyle(Theme.muted)
                }
                Text(StatsText.help)
                    .font(Theme.sans(12)).foregroundStyle(Theme.faint)
                    .lineSpacing(1.5)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                Text(StatsText.none).font(Theme.sans(14)).foregroundStyle(Theme.muted)
            }
        }
        .padding(.horizontal, wide ? 22 : 18).padding(.vertical, wide ? 20 : 16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glass(16, tint: 0.5)
        .accessibilityElement(children: .combine)
    }
}

/// Règle des trames (celle de l'app Mac) : la latence médiane (trait Spectre)
/// et une bande de la médiane au 95e centile, face aux repères d'une image à
/// 160, 120 et 60 Hz (6,25 ; 8,33 ; 16,67 ms). Échelle de 0 à 20 ms, plus si
/// la mesure dépasse.
struct FrameRuler: View {
    let median: Double
    let p95: Double?

    var body: some View {
        let scale = max(20, ((max(median, p95 ?? 0) * 1.25) / 10).rounded(.up) * 10)
        GeometryReader { geo in
            let w = geo.size.width
            let x = { (ms: Double) in CGFloat(min(max(ms, 0), scale) / scale) * w }
            ZStack(alignment: .topLeading) {
                Rectangle().fill(Color.white.opacity(0.16)).frame(width: w, height: 2).offset(y: 16)
                if let p95, p95 > median {
                    RoundedRectangle(cornerRadius: 3)
                        .fill(Theme.accent.opacity(0.2))
                        .overlay(RoundedRectangle(cornerRadius: 3).strokeBorder(Theme.accent.opacity(0.5)))
                        .frame(width: max(4, x(p95) - x(median)), height: 10)
                        .offset(x: x(median), y: 12)
                }
                // Repères : 160 Hz au-dessus, 120 et 60 Hz en dessous ; une
                // étiquette seulement si elle a la place.
                let ticks = [(1000 / 160.0, "160 Hz", true), (1000 / 120.0, "120 Hz", false), (1000 / 60.0, "60 Hz", false)]
                ForEach(ticks, id: \.1) { ms, label, up in
                    let crowded = ticks.contains { $0.2 == up && $0.0 > ms && x($0.0) - x(ms) < 38 }
                    Rectangle().fill(Theme.faint.opacity(0.7)).frame(width: 1, height: 18).offset(x: x(ms), y: 8)
                    if !crowded {
                        Text(verbatim: label).font(Theme.mono(9)).foregroundStyle(Theme.faint)
                            .fixedSize()
                            .frame(width: 44)
                            .offset(x: x(ms) - 22, y: up ? -6 : 28)
                    }
                }
                RoundedRectangle(cornerRadius: 2)
                    .fill(Theme.accent)
                    .frame(width: 3, height: 24)
                    .offset(x: x(median) - 1.5, y: 5)
            }
        }
        .frame(height: 40)
        .padding(.top, 4)
        .accessibilityHidden(true)
    }
}
