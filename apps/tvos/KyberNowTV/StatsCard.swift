// Rubrique « Statistiques » des réglages d'un ordinateur : la carte
// « Dernière session », sur le modèle de celle de l'app Mac (médiane en
// grand, règle des durées d'une image à 160, 120 et 60 Hz, bande de la
// médiane au 95e centile), à la taille du téléviseur.

import KyberKit
import SwiftUI

struct LastSessionCard: View {
    let summary: LastSessionSummary?

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(alignment: .firstTextBaseline) {
                Theme.eyebrow(StatsText.lastSession)
                Spacer()
                if let summary, !summary.when.isEmpty {
                    Text(summary.when).font(Theme.sans(20)).foregroundStyle(Theme.muted)
                }
            }
            if let summary {
                if let median = summary.median {
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Text(verbatim: LinkFormat.decimal(median)).font(Theme.mono(64, .medium)).foregroundStyle(Theme.text)
                        Text(verbatim: "ms \(StatsText.median)").font(Theme.mono(22)).foregroundStyle(Theme.muted)
                    }
                    FrameRule(median: median, p95: summary.p95)
                        .padding(.vertical, 6)
                    if let p95 = summary.p95 {
                        Text(StatsText.under(p95)).font(Theme.sans(21)).foregroundStyle(Theme.text2)
                    }
                }
                if !summary.transmitted.isEmpty {
                    Text(verbatim: summary.transmitted).font(Theme.mono(19)).foregroundStyle(Theme.muted)
                }
                // « Chemin : Direct » ou « Chemin : Par le relais » (rien pour une session d'avant le relais).
                if let path = summary.pathLine {
                    Text(verbatim: path).font(Theme.mono(19)).foregroundStyle(Theme.muted)
                }
                Text(StatsText.help)
                    .font(Theme.sans(18)).foregroundStyle(Theme.faint)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                Text(StatsText.none).font(Theme.sans(24)).foregroundStyle(Theme.text2)
            }
        }
        .padding(.horizontal, 32).padding(.vertical, 28)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 22, style: .continuous).fill(Color.white.opacity(0.05)))
        .overlay(RoundedRectangle(cornerRadius: 22, style: .continuous).strokeBorder(Color.white.opacity(0.08)))
    }
}

/// Règle de trame : la latence lue contre la durée d'une image (160, 120,
/// 60 Hz), de 0 à 20 ms, plus si la mesure dépasse.
struct FrameRule: View {
    var median: Double
    var p95: Double?

    var body: some View {
        let scale = max(20, ((max(median, p95 ?? 0) * 1.25) / 10).rounded(.up) * 10)
        GeometryReader { geo in
            let w = geo.size.width
            let x = { (ms: Double) in CGFloat(min(ms, scale) / scale) * w }
            ZStack(alignment: .topLeading) {
                Rectangle().fill(Color.white.opacity(0.16)).frame(height: 3).offset(y: 34)
                if let p95, p95 > median {
                    RoundedRectangle(cornerRadius: 5)
                        .fill(Theme.accent.opacity(0.22))
                        .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(Theme.accent.opacity(0.55)))
                        .frame(width: max(8, x(p95) - x(median)), height: 18)
                        .offset(x: x(median), y: 26.5)
                }
                // Repères d'une image à 160, 120 et 60 Hz ; une étiquette seulement si elle a la place.
                let ticks = [(1000 / 160.0, "160 Hz", true), (1000 / 120.0, "120 Hz", false), (1000 / 60.0, "60 Hz", false)]
                ForEach(ticks, id: \.1) { ms, label, up in
                    let crowded = ticks.contains { $0.2 == up && $0.0 > ms && x($0.0) - x(ms) < 70 }
                    Rectangle().fill(Theme.faint.opacity(0.7)).frame(width: 2, height: 32).offset(x: x(ms) - 1, y: 20)
                    if !crowded {
                        Text(verbatim: label).font(Theme.mono(16)).foregroundStyle(Theme.faint)
                            .fixedSize()
                            .offset(x: x(ms) - 26, y: up ? -6 : 58)
                    }
                }
                RoundedRectangle(cornerRadius: 3)
                    .fill(Theme.accent)
                    .frame(width: 5, height: 42)
                    .offset(x: x(median) - 2.5, y: 15)
                    .shadow(color: Theme.accentFill, radius: 8)
                // Bornes de l'échelle.
                Text(verbatim: "0").font(Theme.mono(14)).foregroundStyle(Theme.ghost).offset(x: 0, y: 58)
                Text(verbatim: "\(Int(scale)) ms").font(Theme.mono(14)).foregroundStyle(Theme.ghost)
                    .fixedSize()
                    .frame(width: w, alignment: .trailing)
                    .offset(y: 58)
            }
        }
        .frame(height: 84)
        .accessibilityElement()
        .accessibilityLabel(Text(verbatim: "\(StatsText.median) \(LinkFormat.ms(median))"))
    }
}
