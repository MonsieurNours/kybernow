// Les images des ordinateurs : en plein écran sur l'accueil (le décor vit,
// la dernière image respire doucement), en vignette sur les cartes. À défaut
// d'image, la signature de l'ordinateur, tirée de l'empreinte de sa console.

import SwiftUI

/// Image d'un ordinateur : dernière image, aperçu ou signature ; hors ligne, désaturée.
struct HostPicture: View {
    let host: Host
    var online = true
    @EnvironmentObject private var imagery: HostImagery

    var body: some View {
        Group {
            if let picture = imagery.pictures[host.id] {
                Image(uiImage: picture.image)
                    .resizable()
                    .aspectRatio(contentMode: .fill)
            } else {
                Signature(seed: host.consoleFingerprint ?? host.name)
            }
        }
        .saturation(online ? 1 : 0)
        .brightness(online ? 0 : -0.35)
    }
}

/// Le décor de l'accueil : l'image de l'ordinateur choisi, qui dérive
/// lentement (40 s), voilée à gauche et en bas pour le texte.
struct HostBackdrop: View {
    let host: Host
    var online = true
    @State private var drift = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            HostPicture(host: host, online: online)
                .scaleEffect(drift ? 1.05 : 1, anchor: UnitPoint(x: 0.65, y: 0.6))
                .offset(x: drift ? -14 : 0, y: drift ? -8 : 0)
                .frame(width: 1920, height: 1080)
                .clipped()
            LinearGradient(stops: [.init(color: Theme.background.opacity(0.9), location: 0),
                                   .init(color: Theme.background.opacity(0.55), location: 0.4),
                                   .init(color: Theme.background.opacity(0), location: 0.68)],
                           startPoint: .leading, endPoint: .trailing)
            LinearGradient(stops: [.init(color: Theme.background, location: 0),
                                   .init(color: Theme.background.opacity(0.86), location: 0.28),
                                   .init(color: Theme.background.opacity(0), location: 0.56)],
                           startPoint: .bottom, endPoint: .top)
            LinearGradient(colors: [Theme.background.opacity(0.6), .clear], startPoint: .top, endPoint: .bottom)
                .frame(height: 180)
                .frame(maxHeight: .infinity, alignment: .top)
        }
        .frame(width: 1920, height: 1080)
        .onAppear {
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 40).repeatForever(autoreverses: true)) { drift = true }
        }
        .accessibilityHidden(true)
    }
}

// MARK: - Signature

/// Deux lueurs dont la teinte et la place viennent de l'empreinte de la
/// console (dans l'aire du Spectre), et le symbole immense, très sombre.
/// La même que sur le Mac.
struct Signature: View {
    let seed: String

    private var words: [Int] {
        let hex = seed.uppercased().filter { "0123456789ABCDEF".contains($0) }
        var out = stride(from: 0, to: hex.count - 3, by: 4).compactMap { i -> Int? in
            let start = hex.index(hex.startIndex, offsetBy: i)
            return Int(hex[start ..< hex.index(start, offsetBy: 4)], radix: 16)
        }
        if out.count < 4 {
            var h: UInt32 = 2_166_136_261
            for b in seed.utf8 { h = (h ^ UInt32(b)) &* 16_777_619 }
            out = [Int(h & 0xFFFF), Int(h >> 16), Int((h &* 31) & 0xFFFF), Int((h &* 131) >> 16)]
        }
        return out
    }

    private static func hsl(_ h: Double, _ s: Double, _ l: Double, _ a: Double) -> Color {
        let v = l + s * min(l, 1 - l)
        let sv = v == 0 ? 0 : 2 * (1 - l / v)
        return Color(hue: h / 360, saturation: sv, brightness: v).opacity(a)
    }

    var body: some View {
        let w = words
        let h1 = 228 + Double(w[0] % 70), h2 = 228 + Double(w[1] % 70)
        let p1 = UnitPoint(x: 0.55 + Double(w[2] % 30) / 100, y: 0.15 + Double((w[2] >> 8) % 40) / 100)
        let p2 = UnitPoint(x: 0.30 + Double(w[3] % 40) / 100, y: 0.55 + Double((w[3] >> 8) % 35) / 100)
        GeometryReader { geo in
            let side = max(geo.size.width, geo.size.height) * 0.86
            ZStack {
                Theme.background
                RadialGradient(colors: [Self.hsl(h1, 0.78, 0.58, 0.34), .clear], center: p1, startRadius: 0, endRadius: geo.size.width * 0.46)
                RadialGradient(colors: [Self.hsl(h2, 0.70, 0.48, 0.26), .clear], center: p2, startRadius: 0, endRadius: geo.size.width * 0.40)
                ZStack {
                    MarkPart(part: .source).fill(Color.white.opacity(0.045))
                    MarkPart(part: .dest).fill(Self.hsl(h1, 0.70, 0.64, 0.12))
                }
                .frame(width: side, height: side)
                .position(x: geo.size.width * 0.42 + side / 2, y: geo.size.height * 0.5 - side * 0.14)
            }
        }
        .accessibilityHidden(true)
    }
}

/// Fond sombre des écrans secondaires (ajout, réglages, manettes) : deux
/// lueurs Spectre floues et la couture, très discrète.
struct AmbientBackground: View {
    var body: some View {
        ZStack {
            Theme.background
            RadialGradient(colors: [Theme.rgb(0x2A2266), .clear], center: UnitPoint(x: 0.78, y: 0.3), startRadius: 0, endRadius: 1100)
            RadialGradient(colors: [Theme.rgb(0x1A1640), .clear], center: UnitPoint(x: 0.15, y: 0.9), startRadius: 0, endRadius: 900)
            Path { p in
                p.move(to: CGPoint(x: 1980, y: -60))
                p.addLine(to: CGPoint(x: 860, y: 1140))
            }
            .stroke(Theme.accentFill.opacity(0.07 * 0.5), lineWidth: 160)
            Path { p in
                p.move(to: CGPoint(x: 1980, y: -60))
                p.addLine(to: CGPoint(x: 860, y: 1140))
            }
            .stroke(Theme.accent.opacity(0.28 * 0.5), lineWidth: 2)
        }
        .frame(width: 1920, height: 1080)
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }
}
