// Statistiques d'une session, à la demande depuis la barre (maquettes iOS, planche 6). Sur iPhone,
// une feuille compacte à mi-hauteur : latence et débit sur 30 s, le trajet
// d'une image en une barre empilée, pertes, son. Sur iPad, le panneau du Mac
// (apps/macos/KyberNow/SessionStats.swift) : le trajet en cascade sur une
// échelle commune, les courbes, les pertes, l'aller-retour, l'image et le son.
// La vue ne dessine pas son fond : la feuille (iPhone) ou le panneau (iPad)
// de SessionView le pose, opaque, pour ne pas recalculer un flou à chaque image.

import KyberKit
import SwiftUI

struct SessionStatsView: View {
    @ObservedObject var session: SessionController
    /// Largeur donnée à la vue : la mise en page s'y adapte.
    @State private var width: CGFloat = 0

    var body: some View {
        Group {
            if DeviceDisplay.isPad {
                panel
            } else {
                sheet
            }
        }
        .onGeometryChange(for: CGFloat.self, of: { $0.size.width }) { width = $0 }
        .environment(\.colorScheme, .dark)
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Statistiques"))
    }

    // MARK: - Données

    private var stats: VideoStats? { session.stats }
    private var history: [VideoStats] { session.statsHistory }

    /// Durée d'une image au débit du moment (repère des graphiques).
    private var budget: Double { 1000 / max(30, stats?.fps ?? 60) }

    private var latencies: [Double?] { history.map(\.latencyMs) }

    /// 95e centile des médianes de chaque seconde, sur 30 s.
    private var p95: Double? {
        let known = latencies.compactMap { $0 }.sorted()
        guard !known.isEmpty else { return nil }
        return known[min(known.count - 1, Int((Double(known.count) * 0.95).rounded(.down)))]
    }

    /// Secondes avec des pertes : repères sous la courbe de latence.
    private var lossMarks: [Int] { history.map { $0.lostFrames + $0.holes } }

    private var lost30: Int { history.reduce(0) { $0 + $1.lostFrames } }
    private var frames30: Double { history.reduce(0) { $0 + $1.fps } }
    private var keyframes30: Int { history.reduce(0) { $0 + $1.keyframeRequests } }

    /// Images montrées depuis le début, estimées par la cadence moyenne : le
    /// contrôleur ne compte que les pertes.
    private var framesSinceStart: Double {
        let fps = history.isEmpty ? 0 : frames30 / Double(history.count)
        return fps * max(1, Date().timeIntervalSince(session.startedAt))
    }

    private var codecName: String {
        switch session.imageCodec {
        case .h264: return "H.264"
        case .hevc: return "HEVC"
        case .av1: return "AV1"
        }
    }

    /// Débit visé par les réglages pour le mode de la session.
    private var targetMbps: Int {
        let s = session.host.settings
        let mode = DeviceDisplay.mode(for: s)
        return s.bitrate(width: stats?.width ?? mode.width, height: stats?.height ?? mode.height, refresh: mode.refresh)
    }

    private var stages: [StatsStage] {
        guard let s = stats else { return [] }
        let c = s.chain
        // Sans les mesures de l'ordinateur, l'appareil garde les siennes.
        let decode = c?.decode ?? s.decodeMs
        let display = c?.display ?? max(0, s.displayMs - s.decodeMs)
        let here = DeviceDisplay.isPad ? String(localized: "Décompression de l'image par cet iPad") : String(localized: "Décompression de l'image par cet iPhone")
        return [
            StatsStage(name: String(localized: "Capture"), ms: c?.capture, color: Theme.ghost, help: String(localized: "De la capture de l'image au début de son encodage")),
            StatsStage(name: String(localized: "Encodage"), ms: c?.encode, color: Theme.faint, help: String(localized: "Compression de l'image par la carte graphique de l'ordinateur")),
            StatsStage(name: String(localized: "Envoi"), ms: c?.send, color: Theme.muted, help: String(localized: "Mise en paquets et départ sur le réseau")),
            StatsStage(name: String(localized: "Réseau"), ms: c?.network, color: Theme.accentFill, help: String(localized: "Du départ de l'ordinateur à l'image complète reçue ici")),
            StatsStage(name: String(localized: "Décodage"), ms: s.decodeMs > 0 || c != nil ? decode : nil, color: Theme.accent, help: here),
            StatsStage(name: String(localized: "Affichage"), ms: s.displayMs > 0 || c != nil ? display : nil, color: Theme.accentLight, help: String(localized: "Rendu et attente du prochain rafraîchissement de l'écran")),
        ]
    }

    /// « Mesures : 97 % des images · 3 rejetées ».
    private var coverage: String? {
        guard let s = stats, s.latencyMs != nil || s.rejectedSamples > 0 else { return nil }
        var text = String(localized: "Mesures : \(Int((s.measuredRatio * 100).rounded())) % des images")
        if s.rejectedSamples > 0 { text += " · " + String(localized: "\(s.rejectedSamples) rejetées") }
        return text
    }

    // MARK: - Tuiles communes

    private var lossesTile: StatsTile {
        let pct = frames30 > 0 ? Double(lost30) / frames30 * 100 : 0
        return StatsTile(label: String(localized: "Pertes 30 s"), value: "\(lost30) · \(StatsFormat.percent(pct))", warn: lost30 > 0)
    }

    private var sessionTile: StatsTile {
        let n = session.sessionLosses.frames
        let total = framesSinceStart
        let pct = total > 0 ? Double(n) / total * 100 : 0
        return StatsTile(label: String(localized: "Depuis le début"), value: "\(n) · \(StatsFormat.percent(pct))", warn: false)
    }

    private var rttTile: StatsTile {
        StatsTile(label: String(localized: "Aller-retour"), value: stats?.rttMs.map { StatsFormat.ms($0) } ?? "—", warn: false)
    }

    /// « Chemin » : « Direct », « Par le relais » ou « Par le relais (TCP) » (le cœur le dit).
    private var pathTile: StatsTile {
        StatsTile(label: LinkText.path, value: session.path.label, warn: false)
    }

    /// Écart entre le son et l'image : tampon et sortie du son, moins le
    /// trajet de l'image de la réception à l'écran.
    private var audioLag: Double? {
        guard let a = session.audioStats, a.channels > 0, let s = stats else { return nil }
        return a.bufferMs + a.outputMs - s.displayMs
    }

    private func signed(_ v: Double) -> String {
        (v >= 0 ? "+" : "−") + StatsFormat.ms(abs(v))
    }

    private var audioOff: Bool { session.host.settings.audio == .off || (session.audioStats?.channels ?? 0) == 0 }

    // MARK: - iPhone : feuille compacte

    private var sheet: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                sheetHeader
                if stats == nil {
                    waiting
                } else if width >= 640 {
                    HStack(alignment: .top, spacing: 18) {
                        sheetCurves.frame(width: (width - 40 - 36) * 0.42)
                        StatsStackedBar(stages: stages).frame(maxWidth: .infinity)
                        sheetTiles.frame(maxWidth: .infinity)
                    }
                } else {
                    sheetCurves
                    StatsStackedBar(stages: stages)
                    sheetTiles
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 14)
            .padding(.bottom, 18)
        }
        .scrollBounceBehavior(.basedOnSize)
    }

    private var sheetHeader: some View {
        HStack(alignment: .center, spacing: 12) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(verbatim: stats?.latencyMs.map { StatsFormat.ms($0) } ?? "—")
                    .font(Theme.mono(22, .medium))
                    .foregroundStyle(stats?.latencyMs == nil ? Theme.faint : Theme.accentLight)
                    .contentTransition(.numericText())
                Text(verbatim: headerDetail)
                    .font(Theme.mono(11))
                    .foregroundStyle(Theme.muted)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            Spacer(minLength: 8)
            if let coverage, width >= 640 {
                Text(verbatim: coverage).font(Theme.mono(10.5)).foregroundStyle(Theme.muted).lineLimit(1)
            }
            Button {
                session.statsSheet = false
            } label: {
                Icon("x", size: 14, bold: true)
                    .foregroundStyle(Theme.text)
                    .frame(width: 34, height: 34)
                    .background(Theme.surface2, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Theme.line))
                    .frame(width: 44, height: 44)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Text("Fermer les statistiques"))
        }
    }

    /// « médiane · p95 14,1 · 2622 × 1206 · 120 Hz · HEVC ».
    private var headerDetail: String {
        var parts = [String(localized: "médiane")]
        if let p95 { parts.append("p95 " + StatsFormat.ms(p95, unit: false)) }
        if !session.modeLabel.isEmpty { parts.append(session.modeLabel) }
        parts.append(codecName)
        return parts.joined(separator: " · ")
    }

    private var sheetCurves: some View {
        VStack(alignment: .leading, spacing: 8) {
            StatsCurve(title: String(localized: "Latence · 30 s"),
                       caption: String(localized: "\(StatsFormat.ms(budget)) = une image à \(Int((stats?.fps ?? 60).rounded())) i/s"),
                       values: latencies, floorTop: budget * 1.6, guide: budget, marks: lossMarks,
                       fill: false, height: 56)
            StatsCurve(title: String(localized: "Débit · 30 s"),
                       caption: stats.map { StatsFormat.mbps($0.mbps) } ?? "—",
                       values: history.map { $0.mbps }, floorTop: 10, guide: nil, marks: [],
                       fill: true, height: 30)
        }
    }

    private var sheetTiles: some View {
        var tiles = [lossesTile, sessionTile, rttTile, pathTile,
                     StatsTile(label: String(localized: "Images / s"), value: stats.map { "\(Int($0.fps.rounded()))" } ?? "—", warn: false)]
        if audioOff {
            tiles.append(StatsTile(label: String(localized: "Son"), value: String(localized: "sur l'ordinateur"), warn: false))
        } else {
            tiles.append(StatsTile(label: String(localized: "Son · \(session.route.name)"), value: audioLag.map(signed) ?? "—", warn: session.route.isLate || (audioLag ?? 0) > 40))
            tiles.append(StatsTile(label: String(localized: "Tampon son"), value: session.audioStats.map { StatsFormat.ms($0.bufferMs) } ?? "—", warn: false))
        }
        return VStack(alignment: .leading, spacing: 10) {
            StatsTileGrid(tiles: tiles, columns: 2, size: 15)
            if let coverage, width < 640 {
                Text(verbatim: coverage).font(Theme.mono(10.5)).foregroundStyle(Theme.muted)
            }
        }
    }

    private var waiting: some View {
        Text("En attente des premières mesures de l'image.")
            .font(Theme.sans(13))
            .foregroundStyle(Theme.muted)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, 12)
    }

    // MARK: - iPad : le panneau du Mac

    private var panel: some View {
        VStack(alignment: .leading, spacing: 16) {
            panelHeader
            if stats == nil {
                waiting
            } else {
                StatsCascade(stages: stages, budget: budget, fps: stats?.fps ?? 60)
                if width >= 560 {
                    HStack(alignment: .top, spacing: 20) {
                        panelCurves.frame(width: (width - 44 - 20) * 0.56)
                        StatsTileGrid(tiles: panelTiles, columns: 2, size: 16)
                    }
                } else {
                    panelCurves
                    StatsTileGrid(tiles: panelTiles, columns: 3, size: 15)
                }
            }
        }
        .padding(.horizontal, 22)
        .padding(.top, 18)
        .padding(.bottom, 20)
    }

    private var panelHeader: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Theme.eyebrow(String(localized: "Statistiques"), color: Theme.muted)
                if let ms = stats?.latencyMs {
                    Text(verbatim: StatsFormat.ms(ms))
                        .font(Theme.mono(15, .medium))
                        .foregroundStyle(Theme.accentLight)
                        .contentTransition(.numericText())
                }
            }
            Text(verbatim: ([coverage].compactMap { $0 } + [String(localized: "depuis \(StatsFormat.duration(Date().timeIntervalSince(session.startedAt)))")]).joined(separator: " · "))
                .font(Theme.mono(10.5))
                .foregroundStyle(Theme.ghost)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
        }
        // La croix du panneau est posée par SessionView, en haut à droite.
        .padding(.trailing, 40)
        .frame(minHeight: 32, alignment: .topLeading)
    }

    private var panelCurves: some View {
        VStack(alignment: .leading, spacing: 8) {
            StatsCurve(title: String(localized: "Latence · 30 s"),
                       caption: "p50 " + (stats?.latencyMs.map { StatsFormat.ms($0) } ?? "—") + (p95.map { " · p95 " + StatsFormat.ms($0, unit: false) } ?? ""),
                       values: latencies, floorTop: budget * 1.6, guide: budget, marks: lossMarks,
                       fill: false, height: 62)
            StatsCurve(title: String(localized: "Débit · 30 s"),
                       caption: (stats.map { StatsFormat.mbps($0.mbps) } ?? "—") + " · " + String(localized: "cible \(targetMbps)"),
                       values: history.map { $0.mbps }, floorTop: 10, guide: nil, marks: [],
                       fill: true, height: 32)
        }
    }

    private var panelTiles: [StatsTile] {
        let fps = stats.map { Int($0.fps.rounded()) } ?? 0
        let image = keyframes30 == 1 ? String(localized: "\(fps) i/s · 1 clé") : String(localized: "\(fps) i/s · \(keyframes30) clés")
        var tiles = [lossesTile, sessionTile, rttTile, pathTile,
                     StatsTile(label: String(localized: "Image"), value: image, warn: keyframes30 > 0)]
        if audioOff {
            tiles.append(StatsTile(label: String(localized: "Son"), value: String(localized: "sur l'ordinateur"), warn: false))
        } else {
            tiles.append(StatsTile(label: String(localized: "Son · tampon"), value: session.audioStats.map { StatsFormat.ms($0.bufferMs) } ?? "—", warn: (session.audioStats?.gaps ?? 0) > 0))
            tiles.append(StatsTile(label: String(localized: "Écart son − image"), value: audioLag.map(signed) ?? "—", warn: (audioLag ?? 0) > 40))
        }
        return tiles
    }
}

// MARK: - Nombres

/// Nombres des mesures et des réglages : virgule décimale en français, point
/// en anglais, comme sur le Mac.
enum StatsFormat {
    private static var french: Bool { LanguageChoice.effective == "fr" }

    static func decimal(_ value: Double, digits: Int = 1) -> String {
        let text = String(format: "%.\(digits)f", value)
        return french ? text.replacingOccurrences(of: ".", with: ",") : text
    }

    /// Une décimale sous 100 ms.
    static func ms(_ value: Double, unit: Bool = true) -> String {
        let text = value >= 100 ? String(Int(value.rounded())) : decimal(value)
        return unit ? "\(text) ms" : text
    }

    static func mbps(_ value: Double) -> String {
        (value >= 100 ? String(Int(value.rounded())) : decimal(value)) + " Mbit/s"
    }

    /// Pourcentage : deux décimales sous 0,1 %, une au-delà.
    static func percent(_ value: Double) -> String {
        if value == 0 { return "0 %" }
        return decimal(value, digits: value < 0.1 ? 2 : 1) + " %"
    }

    static func duration(_ seconds: TimeInterval) -> String {
        let minutes = Int(seconds / 60)
        if minutes < 60 { return "\(max(1, minutes)) min" }
        return "\(minutes / 60) h " + String(format: "%02d", minutes % 60)
    }
}

// MARK: - Étapes

/// Une étape du trajet d'une image.
struct StatsStage {
    let name: String
    let ms: Double?
    let color: Color
    let help: String
}

/// Le trajet d'une image en une barre empilée (iPhone), avec sa légende.
private struct StatsStackedBar: View {
    let stages: [StatsStage]

    var body: some View {
        let total = stages.compactMap(\.ms).reduce(0, +)
        VStack(alignment: .leading, spacing: 8) {
            StatsLabel(text: String(localized: "Trajet d'une image · médianes"))
            GeometryReader { g in
                HStack(spacing: 0) {
                    ForEach(Array(stages.enumerated()), id: \.offset) { _, stage in
                        if let ms = stage.ms, total > 0 {
                            Rectangle().fill(stage.color).frame(width: g.size.width * ms / total)
                        }
                    }
                }
                .frame(width: g.size.width, alignment: .leading)
                .background(Color.white.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
            }
            .frame(height: 14)
            .accessibilityHidden(true)
            Grid(alignment: .leading, horizontalSpacing: 10, verticalSpacing: 4) {
                ForEach(0 ..< (stages.count + 1) / 2, id: \.self) { row in
                    GridRow {
                        ForEach(0 ..< 2, id: \.self) { col in
                            let i = row * 2 + col
                            if i < stages.count { legend(stages[i]) }
                        }
                    }
                }
            }
            if stages.contains(where: { $0.ms == nil }) {
                Text("L'ordinateur n'envoie pas encore le détail de ses étapes.")
                    .font(Theme.sans(10.5))
                    .foregroundStyle(Theme.faint)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func legend(_ stage: StatsStage) -> some View {
        HStack(spacing: 6) {
            RoundedRectangle(cornerRadius: 2).fill(stage.color).frame(width: 8, height: 8)
            Text(stage.name).font(Theme.sans(10.5)).foregroundStyle(Theme.muted)
            Spacer(minLength: 4)
            Text(verbatim: stage.ms.map { StatsFormat.ms($0, unit: false) } ?? "—")
                .font(Theme.mono(10.5, .medium))
                .foregroundStyle(stage.ms == nil ? Theme.ghost : Theme.text)
        }
        .frame(maxWidth: .infinity)
        .accessibilityElement(children: .combine)
        .accessibilityHint(Text(stage.help))
    }
}

/// Le trajet d'une image en cascade (iPad) : chaque étape commence où la
/// précédente finit, sur une échelle commune de la capture à l'écran. Le
/// pointillé marque la durée d'une image.
private struct StatsCascade: View {
    let stages: [StatsStage]
    let budget: Double
    let fps: Double

    private static let valueWidth: CGFloat = 40

    var body: some View {
        let total = stages.compactMap(\.ms).reduce(0, +)
        let scale = max(total, budget) * 1.06
        let starts = stages.indices.map { i in stages[..<i].compactMap(\.ms).reduce(0, +) }
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                StatsLabel(text: String(localized: "Trajet d'une image · médianes de la seconde"))
                Spacer(minLength: 8)
                StatsLabel(text: String(localized: "pointillé : une image à \(Int(fps.rounded())) i/s"), color: Theme.ghost)
            }
            GeometryReader { g in
                let nameWidth = min(96, g.size.width * 0.22)
                let left = nameWidth + 10
                let barWidth = max(1, g.size.width - left - Self.valueWidth - 8)
                ZStack(alignment: .topLeading) {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(stages.enumerated()), id: \.offset) { i, stage in
                            HStack(spacing: 0) {
                                Text(stage.name)
                                    .font(Theme.sans(11.5))
                                    .foregroundStyle(stage.ms == nil ? Theme.faint : Theme.muted)
                                    .lineLimit(1)
                                    .frame(width: nameWidth, alignment: .leading)
                                    .padding(.trailing, 10)
                                ZStack(alignment: .leading) {
                                    if let ms = stage.ms {
                                        RoundedRectangle(cornerRadius: 3, style: .continuous)
                                            .fill(stage.color)
                                            .frame(width: max(2, barWidth * ms / scale), height: 12)
                                            .offset(x: barWidth * starts[i] / scale)
                                    }
                                }
                                .frame(width: barWidth, alignment: .leading)
                                Text(verbatim: stage.ms.map { StatsFormat.ms($0, unit: false) } ?? "—")
                                    .font(Theme.mono(10.5))
                                    .foregroundStyle(stage.ms == nil ? Theme.ghost : Theme.text)
                                    .frame(width: Self.valueWidth, alignment: .trailing)
                                    .padding(.leading, 8)
                                    .contentTransition(.numericText())
                            }
                            .frame(height: 22)
                            .accessibilityElement(children: .combine)
                            .accessibilityHint(Text(stage.help))
                        }
                    }
                    // Axe, repère d'une image et total, sur l'échelle commune.
                    Path { p in
                        p.move(to: CGPoint(x: left, y: 0))
                        p.addLine(to: CGPoint(x: left, y: CGFloat(stages.count) * 22))
                    }
                    .stroke(Color.white.opacity(0.08), lineWidth: 1)
                    let x = (left + barWidth * budget / scale).rounded() + 0.5
                    Path { p in
                        p.move(to: CGPoint(x: x, y: 0))
                        p.addLine(to: CGPoint(x: x, y: CGFloat(stages.count) * 22 + 4))
                    }
                    .stroke(Theme.accentLight.opacity(0.7), style: StrokeStyle(lineWidth: 1, dash: [3, 4]))
                    Text(verbatim: StatsFormat.ms(budget))
                        .font(Theme.mono(9))
                        .foregroundStyle(Theme.faint)
                        .fixedSize()
                        .frame(width: 80, alignment: .trailing)
                        .position(x: x - 44, y: CGFloat(stages.count) * 22 + 10)
                    if total > 0 {
                        Text(verbatim: StatsFormat.ms(total))
                            .font(Theme.mono(9))
                            .foregroundStyle(Theme.faint)
                            .fixedSize()
                            .frame(width: 80, alignment: .trailing)
                            .position(x: left + barWidth * total / scale - 40, y: CGFloat(stages.count) * 22 + 22)
                            .opacity(abs(total - budget) / scale < 0.12 ? 0 : 1)
                    }
                }
            }
            .frame(height: CGFloat(stages.count) * 22 + 28)
            if stages.contains(where: { $0.ms == nil }) {
                Text("L'ordinateur n'envoie pas encore le détail de ses étapes.")
                    .font(Theme.sans(11))
                    .foregroundStyle(Theme.faint)
            }
        }
    }
}

// MARK: - Courbe

/// Une mesure sur les 30 dernières secondes, la plus récente à droite ; les
/// secondes avec des pertes sont marquées sous la courbe.
private struct StatsCurve: View {
    let title: String
    let caption: String
    let values: [Double?]
    /// Haut minimal de l'échelle (une courbe plate reste lisible).
    let floorTop: Double
    let guide: Double?
    let marks: [Int]
    let fill: Bool
    let height: CGFloat

    private static let slots = 30

    var body: some View {
        let known = values.compactMap { $0 }
        let top = max(floorTop, (known.max() ?? 0) * 1.15)
        VStack(alignment: .leading, spacing: 5) {
            HStack(alignment: .firstTextBaseline) {
                StatsLabel(text: title)
                Spacer(minLength: 6)
                Text(verbatim: caption)
                    .font(Theme.mono(10))
                    .foregroundStyle(Theme.muted)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
                    .contentTransition(.numericText())
            }
            Canvas { ctx, size in
                let marked = marks.contains { $0 > 0 }
                let plotH = size.height - (marked ? 9 : 1)
                let step = size.width / CGFloat(Self.slots - 1)
                let x = { (i: Int) in CGFloat(Self.slots - values.count + i) * step }
                let y = { (v: Double) in plotH - CGFloat(min(v, top) / top) * (plotH - 2) }
                if let guide, guide < top {
                    var g = Path()
                    g.move(to: CGPoint(x: 0, y: y(guide)))
                    g.addLine(to: CGPoint(x: size.width, y: y(guide)))
                    ctx.stroke(g, with: .color(Color.white.opacity(0.14)), style: StrokeStyle(lineWidth: 1, dash: [2, 4]))
                }
                // Courbe, coupée là où la mesure manque.
                var line = Path(), area = Path()
                var open = false, areaStart: CGFloat = 0
                for (i, v) in values.enumerated() {
                    guard let v else {
                        if open {
                            area.addLine(to: CGPoint(x: x(i - 1), y: plotH))
                            area.addLine(to: CGPoint(x: areaStart, y: plotH))
                            area.closeSubpath()
                        }
                        open = false
                        continue
                    }
                    let pt = CGPoint(x: x(i), y: y(v))
                    if open {
                        line.addLine(to: pt)
                        area.addLine(to: pt)
                    } else {
                        line.move(to: pt)
                        area.move(to: CGPoint(x: pt.x, y: plotH))
                        area.addLine(to: pt)
                        areaStart = pt.x
                        open = true
                    }
                }
                if open, let last = values.indices.last {
                    area.addLine(to: CGPoint(x: x(last), y: plotH))
                    area.addLine(to: CGPoint(x: areaStart, y: plotH))
                    area.closeSubpath()
                }
                if fill { ctx.fill(area, with: .color(Theme.accent.opacity(0.18))) }
                ctx.stroke(line, with: .color(Theme.accent), style: StrokeStyle(lineWidth: fill ? 1.2 : 1.6, lineCap: .round, lineJoin: .round))
                if let last = values.indices.last, let v = values[last] {
                    let r: CGFloat = 2.5
                    ctx.fill(Path(ellipseIn: CGRect(x: x(last) - r, y: y(v) - r, width: 2 * r, height: 2 * r)), with: .color(Theme.accentLight))
                }
                // Pertes : un repère sous la seconde concernée.
                for (i, n) in marks.enumerated() where n > 0 {
                    let cx = CGFloat(Self.slots - marks.count + i) * step
                    ctx.fill(Path(CGRect(x: cx - 2, y: size.height - 6, width: 4, height: 6)), with: .color(Theme.danger))
                }
            }
            .frame(height: height)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(title))
        .accessibilityValue(Text(verbatim: caption))
    }
}

// MARK: - Tuiles

struct StatsTile {
    let label: String
    let value: String
    let warn: Bool
}

private struct StatsTileGrid: View {
    let tiles: [StatsTile]
    let columns: Int
    let size: CGFloat

    var body: some View {
        Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 11) {
            ForEach(0 ..< (tiles.count + columns - 1) / columns, id: \.self) { row in
                GridRow {
                    ForEach(0 ..< columns, id: \.self) { col in
                        let i = row * columns + col
                        if i < tiles.count {
                            tile(tiles[i])
                        } else {
                            Color.clear.frame(height: 1)
                        }
                    }
                }
            }
        }
    }

    private func tile(_ t: StatsTile) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            StatsLabel(text: t.label)
            HStack(spacing: 5) {
                if t.warn { Icon("warning", size: 12, bold: true).foregroundStyle(Theme.warn) }
                Text(verbatim: t.value)
                    .font(Theme.mono(size))
                    .foregroundStyle(t.warn ? Theme.warnText : Theme.text)
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                    .contentTransition(.numericText())
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}

/// Étiquette en capitales, à la taille des mesures.
private struct StatsLabel: View {
    let text: String
    var color: Color = Theme.faint

    var body: some View {
        Text(text.uppercased())
            .font(Theme.mono(9.5))
            .tracking(0.8)
            .foregroundStyle(color)
            .lineLimit(1)
            .minimumScaleFactor(0.8)
    }
}
