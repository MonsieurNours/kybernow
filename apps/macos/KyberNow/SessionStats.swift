// Statistiques de la session, en bas à droite de l'image (clic sur les
// mesures de la barre, ou ⌃⌥⌘S) : le trajet d'une image étape par étape, les
// 30 dernières secondes en courbes, et les pertes. Le panneau reste ouvert et
// laisse passer clics et mouvements vers l'ordinateur ; seul son bouton de
// fermeture les prend.

import KyberKit
import SwiftUI

/// Bouton de fermeture du panneau, dans l'espace « chrome » de la session.
struct StatsCloseRectKey: PreferenceKey {
    static let defaultValue: CGRect = .zero
    static func reduce(value: inout CGRect, nextValue: () -> CGRect) {
        let next = nextValue()
        if next != .zero { value = next }
    }
}

struct StatsPanel: View {
    let stats: VideoStats?
    /// Les 30 dernières secondes, la plus récente à la fin.
    let history: [VideoStats]
    let audio: AudioStats?
    /// Pertes depuis le début de la session.
    let sessionLosses: (frames: Int, packets: Int)
    let close: () -> Void

    /// Durée d'une image au débit du moment (repère des graphiques).
    private var budget: Double { 1000 / max(30, stats?.fps ?? 60) }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            header
            if let stats {
                Waterfall(stages: stages(stats), budget: budget, fps: stats.fps)
                rule
                curves
                rule
                losses
                rule
                details(stats)
            } else {
                Text("Les statistiques s'affichent dès que l'image arrive.")
                    .font(Theme.sans(12.5))
                    .foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(18)
        .frame(width: 392)
        // Fond opaque, sans verre dépoli : le panneau reste ouvert sur l'image,
        // et un flou d'arrière-plan serait recalculé à chaque image du flux
        // (160 fois par seconde en 4K), au point de faire tomber la cadence.
        .background {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .fill(Theme.background.opacity(0.94))
                .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Color.white.opacity(0.09)))
        }
        .environment(\.colorScheme, .dark)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Statistiques")
    }

    private var rule: some View { Rectangle().fill(Color.white.opacity(0.08)).frame(height: 1) }

    // MARK: En-tête

    private var header: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Theme.eyebrow(String(localized: "Statistiques"))
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    if let ms = stats?.latencyMs {
                        LitNumber(text: Moment.ms(ms), size: 26)
                    } else {
                        Text(verbatim: "—").font(Theme.mono(26, .medium)).foregroundStyle(Theme.faint)
                    }
                    Text("de la capture\nà votre écran")
                        .font(Theme.sans(11.5))
                        .foregroundStyle(Theme.muted)
                        .lineSpacing(1)
                }
            }
            Spacer(minLength: 0)
            Keycap("⌃⌥⌘S").opacity(0.8).padding(.top, 5)
            Button(action: close) { 
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
                Color.clear.preference(key: StatsCloseRectKey.self, value: g.frame(in: .named(SessionChrome.space)))
            })
        }
    }

    // MARK: Trajet d'une image

    private func stages(_ s: VideoStats) -> [Waterfall.Stage] {
        let c = s.chain
        // Sans les mesures de l'ordinateur, le Mac garde les siennes.
        let decode = c?.decode ?? s.decodeMs
        let display = c?.display ?? max(0, s.displayMs - s.decodeMs)
        return [
            .init(name: String(localized: "Capture"), place: .computer, ms: c?.capture, help: String(localized: "De la capture de l'image au début de son encodage")),
            .init(name: String(localized: "Encodage"), place: .computer, ms: c?.encode, help: String(localized: "Compression de l'image par la carte graphique de l'ordinateur")),
            .init(name: String(localized: "Envoi"), place: .computer, ms: c?.send, help: String(localized: "Mise en paquets et départ sur le réseau")),
            .init(name: String(localized: "Réseau"), place: .network, ms: c?.network, help: String(localized: "Du départ de l'ordinateur à l'image complète reçue ici")),
            .init(name: String(localized: "Décodage"), place: .mac, ms: s.decodeMs > 0 || c != nil ? decode : nil, help: String(localized: "Décompression de l'image par ce Mac")),
            .init(name: String(localized: "Affichage"), place: .mac, ms: s.displayMs > 0 || c != nil ? display : nil, help: String(localized: "Rendu et attente du prochain rafraîchissement de l'écran")),
        ]
    }

    // MARK: 30 secondes

    private var curves: some View {
        VStack(alignment: .leading, spacing: 14) {
            let latencies = history.map(\.latencyMs)
            let losses = history.map { $0.lostFrames + $0.holes }
            Curve(title: String(localized: "Latence"), value: stats?.latencyMs.map { Moment.ms($0) } ?? "—",
                  values: latencies, floorTop: budget * 1.6, guide: budget, guideLabel: String(localized: "1 image"),
                  marks: losses, format: { Moment.ms($0) }, lit: true)
            Curve(title: String(localized: "Débit"), value: stats.map { Self.mbps($0.mbps) } ?? "—",
                  values: history.map { $0.mbps }, floorTop: 10, format: Self.mbps, lit: false)
        }
    }

    static func mbps(_ v: Double) -> String {
        (v >= 100 ? String(Int(v.rounded())) : String(format: "%.1f", v).replacingOccurrences(of: ".", with: LanguageChoice.effective == "fr" ? "," : ".")) + " Mbit/s"
    }

    // MARK: Pertes

    private var losses: some View {
        let lost = history.reduce(0) { $0 + $1.lostFrames }
        let skipped = history.reduce(0) { $0 + $1.skipped }
        let packets = history.reduce(0) { $0 + $1.lostPackets }
        let keyframes = history.reduce(0) { $0 + $1.keyframeRequests }
        return VStack(alignment: .leading, spacing: 10) {
            HStack {
                Theme.eyebrow(String(localized: "Pertes · 30 s"))
                Spacer()
                Text("Session : \(sessionLosses.frames) images, \(sessionLosses.packets) paquets")
                    .font(Theme.mono(10.5))
                    .foregroundStyle(Theme.faint)
            }
            HStack(alignment: .top, spacing: 0) {
                tile(lost, String(localized: "Images perdues"), warn: true, help: String(localized: "Envoyées par l'ordinateur et jamais montrées ici : paquets manquants, ou images écartées en attendant une image clé."))
                tile(packets, String(localized: "Paquets perdus"), warn: true, help: String(localized: "Paquets perdus sur le réseau d'après Kyber. La correction d'erreurs en rattrape la plupart sans perte d'image."))
                tile(keyframes, String(localized: "Images clés"), warn: true, help: String(localized: "Images complètes redemandées à l'ordinateur après une perte."))
                tile(skipped, String(localized: "Remplacées"), warn: false, help: String(localized: "Images remplacées par une plus récente avant d'atteindre l'écran : normal quand l'ordinateur envoie plus d'images que l'écran n'en montre."))
            }
        }
    }

    private func tile(_ value: Int, _ label: String, warn: Bool, help: String) -> some View {
        let alert = warn && value > 0
        return VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 5) {
                if alert { Icon("warning", size: 12, bold: true).foregroundStyle(Theme.warn) }
                Text(verbatim: "\(value)")
                    .font(Theme.mono(17, .medium))
                    .foregroundStyle(alert ? Theme.warnText : value == 0 ? Theme.text2 : Theme.text)
                    .contentTransition(.numericText())
            }
            Text(label).font(Theme.sans(11)).foregroundStyle(Theme.muted)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .help(help)
        .accessibilityElement(children: .combine)
    }

    // MARK: Détails

    private func details(_ s: VideoStats) -> some View {
        return Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 7) {
            row(String(localized: "Réseau"), s.rttMs.map { String(localized: "aller-retour \(Moment.ms($0))") } ?? String(localized: "aller-retour —"))
            row(String(localized: "Image"), String(localized: "\(s.width) × \(s.height) · \(s.format) · \(Int(s.fps.rounded())) i/s"))
            if s.latencyMs != nil || s.rejectedSamples > 0 {
                row(String(localized: "Mesures"), String(localized: "\(Int((s.measuredRatio * 100).rounded())) % des images") + (s.rejectedSamples > 0 ? " · " + String(localized: "\(s.rejectedSamples) rejetées") : ""))
            }
            if let audio, audio.channels > 0 {
                let lag = audio.bufferMs + audio.outputMs - s.displayMs
                row(String(localized: "Son"), String(localized: "tampon \(Moment.ms(audio.bufferMs)) · sortie \(Moment.ms(audio.outputMs))"))
                let sign = lag >= 0 ? "+" : "−"
                row(String(localized: "Synchro"), String(localized: "son \(sign)\(Moment.ms(abs(lag))) après l'image") + (audio.gaps > 0 ? " · " + String(localized: "\(audio.gaps) trous") : ""))
            }
        }
    }

    private func row(_ key: String, _ value: String) -> some View {
        GridRow {
            Text(key).font(Theme.sans(11.5)).foregroundStyle(Theme.faint)
            Text(value).font(Theme.mono(11)).foregroundStyle(Theme.text2)
        }
    }
}

// MARK: - Cascade

/// Le trajet d'une image : chaque étape commence où la précédente finit, sur
/// une échelle commune de la capture à l'écran. Le pointillé marque la durée
/// d'une image.
private struct Waterfall: View {
    enum Place {
        case computer, network, mac

        var label: String {
            switch self {
            case .computer: return String(localized: "Sur l'ordinateur")
            case .network: return String(localized: "Réseau")
            case .mac: return String(localized: "Sur ce Mac")
            }
        }
    }

    struct Stage {
        let name: String
        let place: Place
        let ms: Double?
        let help: String
    }

    let stages: [Stage]
    let budget: Double
    let fps: Double

    private static let nameWidth: CGFloat = 78
    private static let valueWidth: CGFloat = 58

    var body: some View {
        let total = stages.compactMap(\.ms).reduce(0, +)
        let scale = max(total, budget) * 1.08
        let starts = stages.indices.map { i in stages[..<i].compactMap(\.ms).reduce(0, +) }
        VStack(alignment: .leading, spacing: 9) {
            HStack(spacing: 6) {
                Theme.eyebrow(String(localized: "Trajet d'une image"))
                Spacer()
                Path { p in
                    p.move(to: CGPoint(x: 0.5, y: 0))
                    p.addLine(to: CGPoint(x: 0.5, y: 11))
                }
                .stroke(Color.white.opacity(0.4), style: StrokeStyle(lineWidth: 1, dash: [2, 3]))
                .frame(width: 1, height: 11)
                Text("1 image · \(Moment.ms(budget))").font(Theme.mono(10.5)).foregroundStyle(Theme.faint)
            }
            .help("Médianes de la dernière seconde. Le pointillé marque la durée d'une image à \(Int(fps.rounded())) i/s.")
            rows(scale: scale, starts: starts)
            if stages.contains(where: { $0.ms == nil }) {
                Text("L'ordinateur n'envoie pas encore le détail de ses étapes.")
                    .font(Theme.sans(11))
                    .foregroundStyle(Theme.faint)
            }
        }
    }

    private func rows(scale: Double, starts: [Double]) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            ForEach(Array(stages.enumerated()), id: \.offset) { i, stage in
                if i == 0 || stages[i - 1].place != stage.place {
                    Text(stage.place.label.uppercased())
                        .font(Theme.mono(9, .medium))
                        .tracking(0.8)
                        .foregroundStyle(Theme.ghost)
                        .padding(.top, i == 0 ? 0 : 5)
                }
                HStack(spacing: 10) {
                    Text(stage.name)
                        .font(Theme.sans(12))
                        .foregroundStyle(stage.ms == nil ? Theme.faint : Theme.text2)
                        .frame(width: Self.nameWidth, alignment: .leading)
                    GeometryReader { g in
                        let w = g.size.width
                        ZStack(alignment: .leading) {
                            Rectangle().fill(Color.white.opacity(0.05)).frame(height: 1)
                            if let ms = stage.ms {
                                RoundedRectangle(cornerRadius: 2, style: .continuous)
                                    .fill(Theme.accent)
                                    .frame(width: max(2, w * ms / scale), height: 8)
                                    .offset(x: w * starts[i] / scale)
                            }
                        }
                        .frame(maxHeight: .infinity)
                    }
                    .frame(height: 16)
                    Text(stage.ms.map { Moment.ms($0) } ?? "—")
                        .font(Theme.mono(11.5))
                        .foregroundStyle(stage.ms == nil ? Theme.ghost : Theme.text)
                        .frame(width: Self.valueWidth, alignment: .trailing)
                        .contentTransition(.numericText())
                }
                .help(stage.help)
                .accessibilityElement(children: .combine)
            }
        }
        // Repère : une image au débit du moment, en travers des barres.
        .overlay(alignment: .topLeading) {
            GeometryReader { g in
                let left = Self.nameWidth + 10
                let w = g.size.width - left - Self.valueWidth - 10
                let x = (left + w * budget / scale).rounded() + 0.5
                Path { p in
                    p.move(to: CGPoint(x: x, y: 0))
                    p.addLine(to: CGPoint(x: x, y: g.size.height))
                }
                .stroke(Color.white.opacity(0.4), style: StrokeStyle(lineWidth: 1, dash: [2, 3]))
            }
            .allowsHitTesting(false)
        }
    }
}

// MARK: - Courbe

/// Une mesure sur les 30 dernières secondes, la plus récente à droite.
private struct Curve: View {
    let title: String
    let value: String
    let values: [Double?]
    /// Haut minimal de l'échelle (une courbe plate reste lisible).
    var floorTop: Double
    var guide: Double?
    var guideLabel: String?
    /// Secondes avec des pertes : repères sous la courbe.
    var marks: [Int] = []
    var format: (Double) -> String
    var lit: Bool

    private static let slots = 30

    var body: some View {
        let known = values.compactMap { $0 }
        let top = max(floorTop, (known.max() ?? 0) * 1.15)
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline) {
                Text(title).font(Theme.sans(12, .medium)).foregroundStyle(Theme.text2)
                if marks.contains(where: { $0 > 0 }) {
                    HStack(spacing: 4) {
                        Triangle().fill(Theme.warn).frame(width: 7, height: 6)
                        Text("pertes").font(Theme.sans(11)).foregroundStyle(Theme.warnText)
                    }
                    .padding(.leading, 6)
                }
                Spacer()
                Text(value).font(Theme.mono(12, .medium)).foregroundStyle(Theme.text)
                    .contentTransition(.numericText())
            }
            Canvas { ctx, size in
                let plotH = size.height - 8
                let step = size.width / CGFloat(Self.slots - 1)
                let x = { (i: Int) in CGFloat(Self.slots - values.count + i) * step }
                let y = { (v: Double) in plotH - CGFloat(min(v, top) / top) * (plotH - 2) }
                // Base et repère.
                var base = Path()
                base.move(to: CGPoint(x: 0, y: plotH))
                base.addLine(to: CGPoint(x: size.width, y: plotH))
                ctx.stroke(base, with: .color(Color.white.opacity(0.12)), lineWidth: 1)
                if let guide, guide < top {
                    var g = Path()
                    g.move(to: CGPoint(x: 0, y: y(guide)))
                    g.addLine(to: CGPoint(x: size.width, y: y(guide)))
                    ctx.stroke(g, with: .color(Color.white.opacity(0.22)), style: StrokeStyle(lineWidth: 1, dash: [2, 3]))
                }
                // Courbe, coupée là où la mesure manque.
                var line = Path(), area = Path()
                var open = false, areaStart: CGFloat = 0
                for (i, v) in values.enumerated() {
                    guard let v else {
                        if open { area.addLine(to: CGPoint(x: x(i - 1), y: plotH)); area.addLine(to: CGPoint(x: areaStart, y: plotH)); area.closeSubpath() }
                        open = false
                        continue
                    }
                    let pt = CGPoint(x: x(i), y: y(v))
                    if open {
                        line.addLine(to: pt); area.addLine(to: pt)
                    } else {
                        line.move(to: pt); area.move(to: CGPoint(x: pt.x, y: plotH)); area.addLine(to: pt)
                        areaStart = pt.x
                        open = true
                    }
                }
                if open, let last = values.indices.last {
                    area.addLine(to: CGPoint(x: x(last), y: plotH)); area.addLine(to: CGPoint(x: areaStart, y: plotH)); area.closeSubpath()
                }
                let color = lit ? Theme.accent : Theme.text2
                ctx.fill(area, with: .linearGradient(Gradient(colors: [color.opacity(0.22), color.opacity(0)]),
                                                     startPoint: CGPoint(x: 0, y: 0), endPoint: CGPoint(x: 0, y: plotH)))
                if lit {
                    var glow = ctx
                    glow.addFilter(.blur(radius: 4))
                    glow.stroke(line, with: .color(Theme.accentFill.opacity(0.8)), style: StrokeStyle(lineWidth: 3, lineJoin: .round))
                }
                ctx.stroke(line, with: .color(lit ? Theme.accentLight : color), style: StrokeStyle(lineWidth: 2, lineCap: .round, lineJoin: .round))
                // Dernier point.
                if let last = values.indices.last, let v = values[last] {
                    let r: CGFloat = 4
                    ctx.fill(Path(ellipseIn: CGRect(x: x(last) - r, y: y(v) - r, width: 2 * r, height: 2 * r)), with: .color(lit ? Theme.neonCore : Theme.text))
                }
                // Pertes : un repère sous la seconde concernée.
                for (i, n) in marks.enumerated() where n > 0 {
                    let cx = CGFloat(Self.slots - marks.count + i) * step
                    var t = Path()
                    t.move(to: CGPoint(x: cx, y: plotH + 1))
                    t.addLine(to: CGPoint(x: cx + 3.5, y: plotH + 7))
                    t.addLine(to: CGPoint(x: cx - 3.5, y: plotH + 7))
                    t.closeSubpath()
                    ctx.fill(t, with: .color(Theme.warn))
                }
            }
            .frame(height: 52)
            HStack {
                Text(verbatim: "−30 s")
                Spacer()
                if let peak = known.max() { Text("pic \(format(peak))") }
                Spacer()
                Text("maintenant")
            }
            .font(Theme.mono(9.5))
            .foregroundStyle(Theme.ghost)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("\(title) sur 30 secondes")
        .accessibilityValue(value)
    }
}

private struct Triangle: Shape {
    func path(in r: CGRect) -> Path {
        Path { p in
            p.move(to: CGPoint(x: r.midX, y: r.minY))
            p.addLine(to: CGPoint(x: r.maxX, y: r.maxY))
            p.addLine(to: CGPoint(x: r.minX, y: r.maxY))
            p.closeSubpath()
        }
    }
}

#if DEBUG
// MARK: - Aperçu (KN_STATS_PREVIEW=1)

/// Panneau rempli de mesures plausibles, sur une image, pour le régler sans session.
struct StatsPanelPreview: View {
    var body: some View {
        let history: [VideoStats] = (0 ..< 30).map { i in
            var s = VideoStats()
            s.fps = 119.6
            s.width = 2560
            s.height = 1440
            s.format = "4:2:0 8 bits"
            s.decodeMs = 2.1
            s.displayMs = 7.4
            let spike = i == 17 || i == 18
            s.latencyMs = 13.2 + sin(Double(i) / 3) * 0.9 + (spike ? 9 : 0)
            s.measuredRatio = 0.97
            s.rejectedSamples = i % 9 == 0 ? 2 : 0
            var c = ChainTimes()
            (c.capture, c.encode, c.send, c.network, c.decode, c.display) = (0.9, 3.4, 0.6, spike ? 9.6 : 1.3, 2.1, 5.3)
            s.chain = c
            s.lostFrames = i == 17 ? 3 : 0
            s.holes = i == 17 ? 1 : 0
            s.keyframeRequests = i == 17 ? 1 : 0
            s.lostPackets = i == 17 ? 14 : (i == 4 ? 2 : 0)
            s.skipped = i % 4 == 0 ? 1 : 0
            s.rttMs = 0.6
            s.mbps = 62 + sin(Double(i) / 2) * 9
            return s
        }
        var audio = AudioStats()
        audio.channels = 2
        audio.bufferMs = 18
        audio.outputMs = 6
        return ZStack {
            Signature(seed: "7F3A9C21B04E5D88")
            StatsPanel(stats: history.last, history: history, audio: audio, sessionLosses: (3, 16), close: {})
                .padding(20)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
        }
        .frame(minWidth: 1100, minHeight: 820)
        .preferredColorScheme(.dark)
    }
}
#endif
