// Test d'affichage réel : une petite fenêtre (deux secondes à l'écran) avec
// la surface Metal de la session, alimentée par un flux HEVC 60 i/s. On
// compte les images que l'écran confirme avoir montrées (presentedTime).

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

let renderer = VideoRenderer()!
var shown = 0.0, displayMs = 0.0
renderer.onStats = { s in shown += s.fps; displayMs = s.displayMs; print(String(format: "  %.0f i/s affichées, réception → écran %.1f ms, %@", s.fps, s.displayMs, s.format)) }
surface.show(renderer)

let decoder = VideoDecoder()
decoder.setCodec(0x68323635)
decoder.onFrame = { renderer.submit($0) }
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
var i = 0
Timer.scheduledTimer(withTimeInterval: 1.0 / 60, repeats: true) { t in
    guard i < aus.count else { t.invalidate(); return }
    aus[i].withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: i % 60 == 0, pts: UInt64(i)) }
    i += 1
}
DispatchQueue.main.asyncAfter(deadline: .now() + 2.6) {
    print("couche Metal : \(renderer.layer.superlayer === surface.layer ? "dans la surface" : "ABSENTE"), taille \(renderer.layer.frame.size), image \(renderer.layer.drawableSize)")
    print(shown > 30 && displayMs > 0 ? "ok    images montrées à l'écran" : "ÉCHEC rien n'a été montré")
    exit(shown > 30 && displayMs > 0 ? 0 : 1)
}
app.run()
