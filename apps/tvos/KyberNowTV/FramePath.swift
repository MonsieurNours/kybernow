// Trajet d'une image dans les statistiques de session (docs/link-ui.md,
// « Statistiques », « En session : trajet d'une image ») : les six étapes en
// une rangée lisible de loin, regroupées par lieu (« Sur l'ordinateur »,
// « Réseau », « Sur cette Apple TV »), le total de chaque lieu en tête, « — »
// sans mesure, pas d'aide. Médianes de la seconde écoulée, venues du cœur
// (`VideoStats.chain`), comme la cascade de l'app Mac (SessionStats).

import KyberKit
import SwiftUI

struct FramePath: View {
    let stats: VideoStats?

    struct Stage {
        let name: String
        let ms: Double?
    }

    struct Place {
        let name: String
        let stages: [Stage]

        /// Total du lieu : la somme des étapes mesurées, « — » sans aucune.
        var total: Double? {
            let known = stages.compactMap(\.ms)
            return known.isEmpty ? nil : known.reduce(0, +)
        }
    }

    var places: [Place] {
        let c = stats?.chain
        // Sans les mesures de l'ordinateur, l'Apple TV garde les siennes.
        let decodeMs = stats?.decodeMs ?? 0, displayMs = stats?.displayMs ?? 0
        let decode = c?.decode ?? decodeMs
        let display = c?.display ?? max(0, displayMs - decodeMs)
        return [
            Place(name: String(localized: "Sur l'ordinateur"), stages: [
                Stage(name: String(localized: "Capture"), ms: c?.capture),
                Stage(name: String(localized: "Encodage"), ms: c?.encode),
                Stage(name: String(localized: "Envoi"), ms: c?.send),
            ]),
            Place(name: String(localized: "Réseau"), stages: [
                Stage(name: String(localized: "Réseau"), ms: c?.network),
            ]),
            Place(name: String(localized: "Sur cette Apple TV"), stages: [
                Stage(name: String(localized: "Décodage"), ms: decodeMs > 0 || c != nil ? decode : nil),
                Stage(name: String(localized: "Affichage"), ms: displayMs > 0 || c != nil ? display : nil),
            ]),
        ]
    }

    var body: some View {
        HStack(alignment: .top, spacing: 0) {
            ForEach(Array(places.enumerated()), id: \.offset) { i, place in
                if i > 0 {
                    Rectangle().fill(Color.white.opacity(0.14)).frame(width: 1, height: 62)
                        .padding(.horizontal, 26)
                }
                group(place)
            }
        }
        .padding(.horizontal, 30).padding(.vertical, 16)
        .glass(20, tint: 0.55)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(Text("Trajet d'une image"))
    }

    private func group(_ place: Place) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                Theme.eyebrow(place.name)
                Text(verbatim: value(place.total)).font(Theme.mono(18, .medium)).foregroundStyle(Theme.text2)
            }
            HStack(alignment: .firstTextBaseline, spacing: 22) {
                ForEach(Array(place.stages.enumerated()), id: \.offset) { _, stage in
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(verbatim: stage.name).font(Theme.sans(19)).foregroundStyle(Theme.muted)
                        Text(verbatim: stage.ms.map(LinkFormat.decimal) ?? "—")
                            .font(Theme.mono(26, .medium))
                            .foregroundStyle(stage.ms == nil ? Theme.ghost : Theme.text)
                    }
                }
            }
        }
    }

    private func value(_ ms: Double?) -> String {
        ms.map { "\(LinkFormat.decimal($0)) ms" } ?? "—"
    }
}
