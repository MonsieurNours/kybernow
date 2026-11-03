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

    #if DEBUG
    /// Démonstration (`picture:<n>:last|preview`) : une image posée comme une
    /// fin de session ou un aperçu, en mémoire seulement.
    func demoSet(_ cg: CGImage, for host: Host, source: Source) {
        pictures[host.id] = Picture(image: NSImage(cgImage: cg, size: NSSize(width: cg.width, height: cg.height)), date: Date(), source: source)
    }
    #endif

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
///
/// L'image, son zoom très lent (40 s, comme une image qui respire), son
/// arrivée (fondu pour la dernière image, balayage pour un aperçu) et sa mise
/// en gris hors ligne sont des animations de Core Animation (`BackdropLayers`),
/// jouées par le serveur d'affichage : une animation SwiftUI sans fin fait
/// recalculer toute la fenêtre à chaque image (50 à 70 % d'un cœur en
/// démonstration, et un accueil figé au retour d'une session quand le fondu
/// de la dernière image héritait du zoom sans fin).
struct HostBackdrop: View {
    let host: Host
    let picture: HostImagery.Picture?
    var online: Bool?
    /// Zoom très lent (40 s), comme une image qui respire.
    var drift = true
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack {
            Signature(seed: host.consoleFingerprint ?? host.name)
                .saturation(online == false ? 0 : 1)
                .brightness(online == false ? -0.3 : 0)
                .animation(.easeInOut(duration: 0.8), value: online)
            BackdropLayers(picture: picture, drift: drift && !reduceMotion, dimmed: online == false)
        }
        .clipped()
        .accessibilityHidden(true)
    }
}

/// Les couches de l'image de fond (voir `HostBackdrop`).
private struct BackdropLayers: NSViewRepresentable {
    let picture: HostImagery.Picture?
    let drift: Bool
    let dimmed: Bool

    func makeNSView(context: Context) -> BackdropView { BackdropView() }

    func updateNSView(_ view: BackdropView, context: Context) {
        view.show(picture)
        view.setDrift(drift)
        view.setDimmed(dimmed)
    }
}

/// Vue qui porte ses propres couches : une scène qui zoome (40 s, aller et
/// retour, ancrée aux deux tiers), l'image remplissant le cadre sans le
/// dépasser, et sa version grise et assombrie par-dessus, montrée hors ligne
/// (calculée une fois par image).
final class BackdropView: NSView {
    private let stage = CALayer()
    private var current: CALayer?
    private var shown: (date: Date, image: NSImage)?
    /// Première mise à jour : l'image est posée sans animation.
    private var updated = false
    private var drifting = false
    private var dimmed = false
    private static let anchor = CGPoint(x: 0.65, y: 0.4) // 0,6 depuis le haut, la couche n'est pas retournée

    init() {
        super.init(frame: .zero)
        layer = CALayer()
        wantsLayer = true
        layer?.masksToBounds = true
        stage.anchorPoint = Self.anchor
        layer?.addSublayer(stage)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) n'est pas pris en charge") }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func layout() {
        super.layout()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        stage.bounds = bounds
        stage.position = CGPoint(x: bounds.width * Self.anchor.x, y: bounds.height * Self.anchor.y)
        for layer in stage.sublayers ?? [] {
            layer.frame = stage.bounds
            layer.sublayers?.forEach { $0.frame = layer.bounds }
        }
        CATransaction.commit()
    }

    /// Nouvelle image (autre date) : fondu de 1,4 s pour une dernière image,
    /// balayage à 45° pour un aperçu ; aucune au premier affichage.
    func show(_ picture: HostImagery.Picture?) {
        let first = !updated
        updated = true
        guard first || picture?.date != shown?.date || picture?.image !== shown?.image else { return }
        shown = picture.map { ($0.date, $0.image) }
        let old = current
        guard let picture else {
            current = nil
            fadeOut(old)
            return
        }
        let layer = CALayer()
        layer.contents = picture.image
        layer.contentsGravity = .resizeAspectFill
        layer.masksToBounds = true
        layer.frame = stage.bounds
        let gray = CALayer()
        gray.contents = Self.dimmed(picture.image)
        gray.contentsGravity = .resizeAspectFill
        gray.frame = layer.bounds
        gray.opacity = dimmed ? 1 : 0
        layer.addSublayer(gray)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        stage.addSublayer(layer)
        CATransaction.commit()
        current = layer
        guard !first, window != nil else { old?.removeFromSuperlayer(); return }
        if picture.source == .preview {
            reveal(layer, over: old)
        } else {
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 0
            fade.toValue = 1
            fade.duration = 1.4
            fade.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            CATransaction.begin()
            CATransaction.setCompletionBlock { old?.removeFromSuperlayer() }
            layer.add(fade, forKey: "arrivée")
            CATransaction.commit()
        }
    }

    /// Arrivée d'un aperçu en balayage à 45°, du haut-gauche vers le
    /// bas-droite (le sens du signal), avec un front de néon.
    private func reveal(_ layer: CALayer, over old: CALayer?) {
        let size = stage.bounds.size
        let reach = (size.width + size.height) * 1.05
        // Couche non retournée : le haut est à y = hauteur.
        func triangle(_ c: CGFloat) -> CGPath {
            let p = CGMutablePath()
            p.move(to: CGPoint(x: 0, y: size.height))
            p.addLine(to: CGPoint(x: c, y: size.height))
            p.addLine(to: CGPoint(x: 0, y: size.height - c))
            p.closeSubpath()
            return p
        }
        func front(_ c: CGFloat) -> CGPath {
            let p = CGMutablePath()
            p.move(to: CGPoint(x: c, y: size.height))
            p.addLine(to: CGPoint(x: 0, y: size.height - c))
            return p
        }
        let mask = CAShapeLayer()
        mask.frame = stage.bounds
        mask.path = triangle(reach)
        layer.mask = mask
        let line = CAShapeLayer()
        line.frame = stage.bounds
        line.path = front(reach)
        line.strokeColor = NSColor(Theme.neonCore).cgColor
        line.fillColor = nil
        line.lineWidth = 3
        line.shadowColor = NSColor(Theme.accent).cgColor
        line.shadowRadius = 10
        line.shadowOpacity = 1
        line.shadowOffset = .zero
        line.opacity = 0
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        stage.addSublayer(line)
        CATransaction.commit()

        let timing = CAMediaTimingFunction(name: .easeInEaseOut)
        func sweep(_ from: CGPath, _ to: CGPath) -> CABasicAnimation {
            let a = CABasicAnimation(keyPath: "path")
            a.fromValue = from
            a.toValue = to
            a.duration = 1.4
            a.timingFunction = timing
            return a
        }
        let glow = CAKeyframeAnimation(keyPath: "opacity")
        glow.values = [0, 1, 1, 0]
        glow.keyTimes = [0, 0.02, 0.96, 1]
        glow.duration = 1.4
        CATransaction.begin()
        CATransaction.setCompletionBlock { [weak layer] in
            layer?.mask = nil
            line.removeFromSuperlayer()
            old?.removeFromSuperlayer()
        }
        mask.add(sweep(triangle(0), triangle(reach)), forKey: "balayage")
        line.add(sweep(front(0), front(reach)), forKey: "balayage")
        line.add(glow, forKey: "lueur")
        CATransaction.commit()
    }

    private func fadeOut(_ layer: CALayer?) {
        guard let layer else { return }
        CATransaction.begin()
        CATransaction.setAnimationDuration(window == nil ? 0 : 1.4)
        CATransaction.setCompletionBlock { layer.removeFromSuperlayer() }
        layer.opacity = 0
        CATransaction.commit()
    }

    /// Zoom très lent, joué par le serveur d'affichage (rien à faire pour l'app).
    func setDrift(_ on: Bool) {
        guard on != drifting else { return }
        drifting = on
        if on {
            let zoom = CABasicAnimation(keyPath: "transform.scale")
            zoom.fromValue = 1
            zoom.toValue = 1.05
            zoom.duration = 40
            zoom.autoreverses = true
            zoom.repeatCount = .infinity
            zoom.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            zoom.isRemovedOnCompletion = false
            stage.add(zoom, forKey: "dérive")
        } else {
            stage.removeAnimation(forKey: "dérive")
        }
    }

    /// Hors ligne : l'image passe en gris et s'assombrit (0,8 s).
    func setDimmed(_ on: Bool) {
        guard on != dimmed else { return }
        dimmed = on
        guard let gray = current?.sublayers?.first else { return }
        CATransaction.begin()
        CATransaction.setAnimationDuration(window == nil ? 0 : 0.8)
        CATransaction.setAnimationTimingFunction(CAMediaTimingFunction(name: .easeInEaseOut))
        gray.opacity = on ? 1 : 0
        CATransaction.commit()
    }

    /// L'image sans couleur et assombrie de 0,3 (comme `saturation(0)` et
    /// `brightness(-0.3)` de SwiftUI).
    private static let context = CIContext()
    private static func dimmed(_ image: NSImage) -> CGImage? {
        guard let cg = image.cgImage(forProposedRect: nil, context: nil, hints: nil),
              let filter = CIFilter(name: "CIColorControls") else { return nil }
        let input = CIImage(cgImage: cg)
        filter.setValue(input, forKey: kCIInputImageKey)
        filter.setValue(0.0, forKey: kCIInputSaturationKey)
        filter.setValue(-0.3, forKey: kCIInputBrightnessKey)
        guard let output = filter.outputImage else { return nil }
        return context.createCGImage(output, from: input.extent)
    }
}
