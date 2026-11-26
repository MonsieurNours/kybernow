// Lecteur de l'écran à l'écran (docs/banc-de-mesure.md), côté appareil (Mac).
//
// Il cherche la bande de la sonde sur les écrans de ce Mac (dans la fenêtre de
// l'app KyberNow Link, ou dans un navigateur pour le plancher), puis capture
// cet écran par ScreenCaptureKit aussi vite qu'il se compose. Pour chaque image
// capturée, il écrit une ligne JSON : l'instant où l'écran l'a montrée (horloge
// murale, ms) et le compteur lu, ou rien si la bande n'est pas lisible.
//
//   lecteur --duree 20 > lectures.jsonl
//
// Construit par `banc.py` (swiftc -O). Il lui faut le droit d'enregistrer
// l'écran (Réglages Système › Confidentialité › Enregistrement de l'écran),
// accordé à l'app qui le lance (le Terminal).
import CoreMedia
import CoreVideo
import Foundation
import ScreenCaptureKit

let bits = 11
var duree = 20.0
var args = CommandLine.arguments.dropFirst()
while let a = args.popFirst() {
    if a == "--duree", let v = args.popFirst(), let d = Double(v) { duree = d }
}

func journal(_ s: String) { FileHandle.standardError.write((s + "\n").data(using: .utf8)!) }

// Horloge murale depuis le temps mach des images.
var base = mach_timebase_info_data_t()
mach_timebase_info(&base)
let mach0 = mach_absolute_time()
let mur0 = Date().timeIntervalSince1970 * 1000
func murale(_ mach: UInt64) -> Double {
    let d = Double(Int64(bitPattern: mach &- mach0)) * Double(base.numer) / Double(base.denom)
    return mur0 + d / 1e6
}

/// Une image BGRA à lire.
struct Pixels {
    let p: UnsafePointer<UInt8>
    let largeur: Int, hauteur: Int, ligne: Int
    func at(_ x: Int, _ y: Int) -> (r: Int, g: Int, b: Int) {
        let o = y * ligne + x * 4
        return (Int(p[o + 2]), Int(p[o + 1]), Int(p[o]))
    }
    enum Teinte { case jaune, bleu, autre }
    func teinte(_ x: Int, _ y: Int) -> Teinte {
        let c = at(x, y)
        if c.r > 150 && c.g > 150 && c.b < 110 { return .jaune }
        if c.b > 140 && c.r < 90 && c.g < 90 { return .bleu }
        return .autre
    }
    /// Moyenne de la luminance au centre d'un bloc (3 × 3 pixels).
    func clair(_ x: Int, _ y: Int) -> Bool {
        var s = 0
        for dy in -1...1 { for dx in -1...1 {
            let c = at(min(max(x + dx, 0), largeur - 1), min(max(y + dy, 0), hauteur - 1))
            s += c.r + c.g + c.b
        } }
        return s / 27 > 128
    }
}

/// Où est la bande : bord gauche, largeur d'un bloc, ligne du milieu.
struct Bande { var x0: Double; var bloc: Double; var y: Int }

func centre(_ b: Bande, _ i: Int) -> Int { Int(b.x0 + (Double(i) + 0.5) * b.bloc) }

func valide(_ px: Pixels, _ b: Bande) -> Bool {
    let xs = [0, 1, 14, 15].map { centre(b, $0) }
    guard xs.last! < px.largeur, b.y >= 0, b.y < px.hauteur else { return false }
    return px.teinte(xs[0], b.y) == .jaune && px.teinte(xs[1], b.y) == .bleu
        && px.teinte(xs[2], b.y) == .bleu && px.teinte(xs[3], b.y) == .jaune
}

/// Cherche la bande dans toute l'image : une suite jaune puis bleu de même
/// largeur, confirmée par les repères de l'autre bout.
func chercher(_ px: Pixels) -> Bande? {
    var y = 0
    while y < px.hauteur {
        var x = 0
        while x < px.largeur {
            guard px.teinte(x, y) == .jaune else { x += 1; continue }
            var fin = x
            while fin < px.largeur && px.teinte(fin, y) == .jaune { fin += 1 }
            var finB = fin
            while finB < px.largeur && px.teinte(finB, y) == .bleu { finB += 1 }
            let lj = fin - x, lb = finB - fin
            if lj >= 6 && lb >= 6 && abs(lj - lb) <= max(3, lj / 4) {
                // Milieu vertical du bloc jaune.
                let cx = x + lj / 2
                var haut = y, bas = y
                while haut > 0 && px.teinte(cx, haut - 1) == .jaune { haut -= 1 }
                while bas < px.hauteur - 1 && px.teinte(cx, bas + 1) == .jaune { bas += 1 }
                let b = Bande(x0: Double(x), bloc: Double(lj + lb) / 2, y: (haut + bas) / 2)
                if valide(px, b) { return b }
            }
            x = max(finB, x + 1)
        }
        y += 3
    }
    return nil
}

/// Le compteur, et si la parité est fausse (image déchirée entre deux).
func lire(_ px: Pixels, _ b: Bande) -> (Int, Bool) {
    var v = 0, uns = 0
    for i in 0...bits {
        let un = px.clair(centre(b, 2 + i), b.y)
        if un { uns += 1 }
        if i < bits { v = v << 1 | (un ? 1 : 0) }
    }
    var s = v >> 1
    while s != 0 { v ^= s; s >>= 1 }
    return (v, uns % 2 != 0)
}

final class Lecteur: NSObject, SCStreamOutput, SCStreamDelegate {
    var bande: Bande?
    var lues = 0, sans = 0
    let sortie = FileHandle.standardOutput

    func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, sb.isValid,
              let infos = CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let info = infos.first,
              let brut = info[.status] as? Int, SCFrameStatus(rawValue: brut) == .complete,
              let image = sb.imageBuffer
        else { return }
        let quand = (info[.displayTime] as? UInt64).map(murale) ?? Date().timeIntervalSince1970 * 1000
        CVPixelBufferLockBaseAddress(image, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(image, .readOnly) }
        guard let adresse = CVPixelBufferGetBaseAddress(image) else { return }
        let px = Pixels(
            p: adresse.assumingMemoryBound(to: UInt8.self),
            largeur: CVPixelBufferGetWidth(image), hauteur: CVPixelBufferGetHeight(image),
            ligne: CVPixelBufferGetBytesPerRow(image))
        if bande == nil || !valide(px, bande!) { bande = chercher(px) }
        var ligne = String(format: "{\"t\":%.3f", quand)
        if let b = bande {
            let (n, dechire) = lire(px, b)
            ligne += ",\"n\":\(n),\"dechire\":\(dechire),\"bloc\":\(String(format: "%.1f", b.bloc))"
            lues += 1
        } else {
            sans += 1
        }
        sortie.write((ligne + "}\n").data(using: .utf8)!)
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        journal("Capture arrêtée : \(error.localizedDescription)")
        exit(1)
    }
}

/// L'écran qui montre la bande : une capture de chacun, le premier où elle se lit.
func ecranDeLaBande(_ ecrans: [SCDisplay]) async -> SCDisplay? {
    for e in ecrans {
        let filtre = SCContentFilter(display: e, excludingWindows: [])
        let c = SCStreamConfiguration()
        c.width = Int(filtre.contentRect.width * CGFloat(filtre.pointPixelScale))
        c.height = Int(filtre.contentRect.height * CGFloat(filtre.pointPixelScale))
        c.pixelFormat = kCVPixelFormatType_32BGRA
        c.showsCursor = false
        guard let img = try? await SCScreenshotManager.captureSampleBuffer(contentFilter: filtre, configuration: c),
              let buf = img.imageBuffer else { continue }
        CVPixelBufferLockBaseAddress(buf, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(buf, .readOnly) }
        let px = Pixels(
            p: CVPixelBufferGetBaseAddress(buf)!.assumingMemoryBound(to: UInt8.self),
            largeur: CVPixelBufferGetWidth(buf), hauteur: CVPixelBufferGetHeight(buf),
            ligne: CVPixelBufferGetBytesPerRow(buf))
        if let b = chercher(px) {
            journal("Bande trouvée sur l'écran \(e.displayID) (\(px.largeur) × \(px.hauteur)), bloc de \(Int(b.bloc)) pixels")
            return e
        }
    }
    return nil
}

let lecteur = Lecteur()
Task {
    do {
        let contenu = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard let ecran = await ecranDeLaBande(contenu.displays) else {
            journal("Bande de la sonde introuvable sur les écrans de ce Mac : la session est-elle au premier plan, la sonde affichée ?")
            exit(3)
        }
        let filtre = SCContentFilter(display: ecran, excludingWindows: [])
        let c = SCStreamConfiguration()
        c.width = Int(filtre.contentRect.width * CGFloat(filtre.pointPixelScale))
        c.height = Int(filtre.contentRect.height * CGFloat(filtre.pointPixelScale))
        c.pixelFormat = kCVPixelFormatType_32BGRA
        c.minimumFrameInterval = CMTime(value: 1, timescale: 240)
        c.queueDepth = 6
        c.showsCursor = false
        let flux = SCStream(filter: filtre, configuration: c, delegate: lecteur)
        try flux.addStreamOutput(lecteur, type: .screen, sampleHandlerQueue: DispatchQueue(label: "banc.lecteur"))
        try await flux.startCapture()
        journal("Capture de \(Int(duree)) s…")
        try await Task.sleep(nanoseconds: UInt64(duree * 1e9))
        try await flux.stopCapture()
        journal("\(lecteur.lues) images lues, \(lecteur.sans) sans bande")
        exit(0)
    } catch {
        journal("Capture impossible : \(error.localizedDescription) (droit d'enregistrer l'écran ?)")
        exit(2)
    }
}
RunLoop.main.run()
