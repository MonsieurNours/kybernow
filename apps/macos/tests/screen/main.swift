// Test d'affichage réel : une petite fenêtre (deux secondes à l'écran par
// passage) avec la surface Metal de la session, alimentée par un flux HEVC
// 60 i/s. On compte les images que l'écran confirme avoir montrées
// (presentedTime), et on compare le délai commit → écran avec et sans la
// garde « une image en vol » (KN_INFLIGHT_GUARD).

import AppKit
import CoreVideo

enum KyberLog {
    static let video = os.Logger(subsystem: "test", category: "video")
}
import os

let path = CommandLine.arguments[1]
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 480, height: 270), styleMask: [.titled], backing: .buffered, defer: false)
window.title = "Test KyberNow : affichage"
let parent = NSView(frame: window.contentLayoutRect)
parent.wantsLayer = true // comme la vue de session : AppKit gère déjà les couches
window.contentView = parent
let surface = MetalSurface(frame: parent.bounds)
surface.autoresizingMask = [.width, .height]
parent.addSubview(surface)
window.orderFrontRegardless()

let data = try! Data(contentsOf: URL(fileURLWithPath: path))
var aus: [[UInt8]] = []
data.withUnsafeBytes { raw in
    var au: [UInt8] = []
    for n in VideoDecoder.nalUnits(raw.bindMemory(to: UInt8.self)) {
        if (n[0] >> 1) & 0x3F == 35, !au.isEmpty { aus.append(au); au = [] }
        au += [0, 0, 0, 1] + Array(n)
    }
    aus.append(au)
}

struct Pass {
    var shown = 0.0, displayMs = 0.0
    var lags: [Double] = [], p95: [Double] = [], discarded = 0, rendered = 0
}
var results: [Pass] = []

/// Un passage du flux (2,6 s), avec ou sans la garde.
func pass(guard inFlightGuard: Bool, then next: @escaping () -> Void) {
    let renderer = VideoRenderer()!
    renderer.inFlightGuard = inFlightGuard
    var p = Pass()
    renderer.onStats = { s in p.shown += s.fps; p.displayMs = s.displayMs }
    renderer.onPresentStats = { s in
        p.lags.append(s.p50Ms); p.p95.append(s.p95Ms); p.discarded += s.discarded; p.rendered += s.rendered
    }
    surface.show(renderer)
    let decoder = VideoDecoder()
    decoder.setCodec(0x68323635)
    decoder.onFrame = { renderer.submit($0) }
    var i = 0
    // Le flux (120 images) est rejoué en boucle : 2,5 s à 60 i/s, deux relevés par passage.
    Timer.scheduledTimer(withTimeInterval: 1.0 / 60, repeats: true) { t in
        guard i < 150 else { t.invalidate(); return }
        aus[i % aus.count].withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: i % 60 == 0, pts: UInt64(i)) }
        i += 1
    }
    DispatchQueue.main.asyncAfter(deadline: .now() + 2.8) {
        let p50 = p.lags.isEmpty ? 0 : p.lags.reduce(0, +) / Double(p.lags.count)
        let p95 = p.p95.isEmpty ? 0 : p.p95.max()!
        print(String(format: "  garde une image en vol %@ : %.0f images montrées, réception → écran %.1f ms, commit → écran p50 %.1f ms, p95 %.1f ms, %d rendues, %d écartées par l'écran",
                     inFlightGuard ? "oui" : "non", p.shown, p.displayMs, p50, p95, p.rendered, p.discarded))
        if results.isEmpty {
            print("couche Metal : \(renderer.layer.superlayer === surface.layer ? "dans la surface" : "ABSENTE"), taille \(renderer.layer.frame.size), image \(renderer.layer.drawableSize)")
        }
        results.append(p)
        decoder.invalidate()
        next()
    }
}

pass(guard: true) {
    pass(guard: false) {
        let ok = results.allSatisfy { $0.shown > 30 && $0.displayMs > 0 }
        print(ok ? "ok    images montrées à l'écran, avec et sans la garde" : "ÉCHEC rien n'a été montré")
        exit(ok ? 0 : 1)
    }
}
app.run()
