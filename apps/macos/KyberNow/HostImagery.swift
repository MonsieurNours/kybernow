// L'image d'un ordinateur sur l'accueil, dans cet ordre : la dernière image
// de sa dernière session (gardée sur ce Mac, désactivable), un aperçu à jour
// demandé à sa console, sinon sa signature, tirée de l'empreinte de sa
// console. Hors ligne, l'image se désature.

import AppKit
import KyberKit
import SwiftUI

@MainActor
final class HostImagery: ObservableObject {
    enum Source: Equatable { case lastSession, preview }

    struct Picture {
        let image: NSImage
        let date: Date
        let source: Source
    }

    @Published private(set) var pictures: [UUID: Picture] = [:]
    /// Aperçus demandés en ce moment.
    @Published private(set) var fetching: Set<UUID> = []
    private var lastFetch: [UUID: Date] = [:]
    private let folder: URL

    init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        folder = base.appendingPathComponent("KyberNow/Images", isDirectory: true)
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let files = (try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        // La dernière image passe avant l'aperçu.
        for url in files.sorted(by: { $0.lastPathComponent.hasSuffix("-last.jpg") && !$1.lastPathComponent.hasSuffix("-last.jpg") }) {
            let name = url.deletingPathExtension().lastPathComponent
            guard let dash = name.lastIndex(of: "-"), let id = UUID(uuidString: String(name[..<dash])),
                  pictures[id] == nil, let image = NSImage(contentsOf: url) else { continue }
            let date = (try? url.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate ?? Date()
            pictures[id] = Picture(image: image, date: date, source: name.hasSuffix("-last") ? .lastSession : .preview)
        }
    }

    private func url(_ id: UUID, _ source: Source) -> URL {
        folder.appendingPathComponent("\(id.uuidString)-\(source == .lastSession ? "last" : "preview").jpg")
    }

    /// Faut-il demander un aperçu ? Pas de dernière image, et pas plus d'un par minute.
    func wantsPreview(_ host: Host) -> Bool {
        if pictures[host.id]?.source == .lastSession { return false }
        return Date().timeIntervalSince(lastFetch[host.id] ?? .distantPast) > 60
    }

    func fetchPreview(for host: Host, console: ConsoleClient) async {
        lastFetch[host.id] = Date()
        fetching.insert(host.id)
        defer { fetching.remove(host.id) }
        guard let shot = await console.preview(width: 1600), let image = NSImage(data: shot.jpeg),
              pictures[host.id]?.source != .lastSession
        else { return }
        let file = url(host.id, .preview)
        try? shot.jpeg.write(to: file, options: .atomic)
        try? FileManager.default.setAttributes([.modificationDate: shot.capturedAt], ofItemAtPath: file.path)
        pictures[host.id] = Picture(image: image, date: shot.capturedAt, source: .preview)
    }

    /// Fin de session : la dernière image devient le fond de l'ordinateur sur l'accueil.
    func saveLast(_ cg: CGImage, for host: Host) {
        guard host.settings.keepLastImage,
              let data = NSBitmapImageRep(cgImage: cg).representation(using: .jpeg, properties: [.compressionFactor: 0.82])
        else { return }
        try? data.write(to: url(host.id, .lastSession), options: .atomic)
        try? FileManager.default.removeItem(at: url(host.id, .preview))
        pictures[host.id] = Picture(image: NSImage(cgImage: cg, size: NSSize(width: cg.width, height: cg.height)), date: Date(), source: .lastSession)
    }

    /// « Garder la dernière image » désactivé : on l'efface tout de suite.
    func forgetLast(_ host: Host) {
        try? FileManager.default.removeItem(at: url(host.id, .lastSession))
        if pictures[host.id]?.source == .lastSession { pictures[host.id] = nil }
        lastFetch[host.id] = nil
    }

    /// Ordinateur oublié : ses images aussi.
    func forget(_ host: Host) {
        try? FileManager.default.removeItem(at: url(host.id, .lastSession))
        try? FileManager.default.removeItem(at: url(host.id, .preview))
        pictures[host.id] = nil
    }
}

// MARK: - Signature

/// Signature d'un ordinateur : deux lueurs dont la teinte et la place viennent
/// de l'empreinte de sa console (dans l'aire du Spectre), et le symbole
/// immense, très sombre, qui déborde à droite. La même sur tous les appareils.
struct Signature: View {
    /// Empreinte de la console (à défaut, le nom).
    let seed: String

    private var words: [Int] {
        let hex = seed.uppercased().filter { "0123456789ABCDEF".contains($0) }
        var out = stride(from: 0, to: hex.count - 3, by: 4).compactMap { i -> Int? in
            let start = hex.index(hex.startIndex, offsetBy: i)
            return Int(hex[start ..< hex.index(start, offsetBy: 4)], radix: 16)
        }
        if out.count < 4 {
            // Pas d'empreinte : on part du nom.
            var h: UInt32 = 2_166_136_261
            for b in seed.utf8 { h = (h ^ UInt32(b)) &* 16_777_619 }
            out = [Int(h & 0xFFFF), Int(h >> 16), Int((h &* 31) & 0xFFFF), Int((h &* 131) >> 16)]
        }
        return out
    }

    /// Couleur depuis une teinte en degrés (TSL → TSV de SwiftUI).
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

// MARK: - Fond de l'accueil

/// Image d'un ordinateur en plein cadre : dernière image, aperçu ou signature.
struct HostBackdrop: View {
    let host: Host
    let picture: HostImagery.Picture?
    var online: Bool?
    /// Zoom très lent (40 s), comme une image qui respire.
    var drift = true
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var zoom = false

    var body: some View {
        // L'image remplit le cadre sans jamais l'agrandir : la mise en page
        // de l'écran ne dépend pas de la taille de l'image.
        GeometryReader { geo in
            ZStack {
                Signature(seed: host.consoleFingerprint ?? host.name)
                if let picture {
                    Image(nsImage: picture.image)
                        .resizable()
                        .interpolation(.high)
                        .scaledToFill()
                        .frame(width: geo.size.width, height: geo.size.height)
                        .clipped()
                        .id(picture.date)
                        .transition(picture.source == .preview ? .diagonalReveal : .opacity)
                }
            }
            .frame(width: geo.size.width, height: geo.size.height)
        }
        .scaleEffect(zoom ? 1.05 : 1, anchor: UnitPoint(x: 0.65, y: 0.6))
        .saturation(online == false ? 0 : 1)
        .brightness(online == false ? -0.3 : 0)
        .clipped()
        .animation(.easeInOut(duration: 1.4), value: picture?.date)
        .animation(.easeInOut(duration: 0.8), value: online)
        .onAppear {
            guard drift, !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 40).repeatForever(autoreverses: true)) { zoom = true }
        }
        .accessibilityHidden(true)
    }
}

/// Arrivée d'une image en balayage à 45°, du haut-gauche vers le bas-droite
/// (le sens du signal), avec un front de néon.
struct DiagonalReveal: ViewModifier, Animatable {
    var progress: CGFloat
    var animatableData: CGFloat {
        get { progress }
        set { progress = newValue }
    }

    func body(content: Content) -> some View {
        content
            .mask {
                GeometryReader { geo in
                    let c = progress * (geo.size.width + geo.size.height) * 1.05
                    Path { p in
                        p.move(to: .zero)
                        p.addLine(to: CGPoint(x: c, y: 0))
                        p.addLine(to: CGPoint(x: 0, y: c))
                        p.closeSubpath()
                    }
                }
            }
            .overlay {
                GeometryReader { geo in
                    let c = progress * (geo.size.width + geo.size.height) * 1.05
                    Path { p in
                        p.move(to: CGPoint(x: c, y: 0))
                        p.addLine(to: CGPoint(x: 0, y: c))
                    }
                    .stroke(Theme.neonCore, lineWidth: 3)
                    .shadow(color: Theme.accent, radius: 6)
                    .shadow(color: Theme.accentFill, radius: 22)
                    .opacity(progress > 0.02 && progress < 0.98 ? 1 : 0)
                }
                .allowsHitTesting(false)
            }
    }
}

extension AnyTransition {
    static var diagonalReveal: AnyTransition {
        .asymmetric(
            insertion: .modifier(active: DiagonalReveal(progress: 0), identity: DiagonalReveal(progress: 1)),
            removal: .opacity
        )
    }
}
