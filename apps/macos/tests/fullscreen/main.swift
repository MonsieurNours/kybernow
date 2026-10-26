// Multi-écrans, fenêtre d'un autre écran (docs/multi-ecrans.md) : ses
// premières images arrivent pendant qu'elle est vue occultée (passage en
// plein écran) et son rendu attend ; puis elle passe en plein écran et plus
// aucune image n'arrive (un bureau immobile n'en envoie pas). Il faut qu'une
// image soit montrée à la taille du plein écran : rendue à la reprise, puis
// redessinée à la nouvelle taille et à la fin du passage.

import AppKit
import CoreVideo
import os

enum KyberLog {
    static let video = os.Logger(subsystem: "test", category: "video")
}

let path = CommandLine.arguments[1]
let app = NSApplication.shared
app.setActivationPolicy(.regular)
// Un autre écran que celui de la barre des menus, s'il y en a un.
let screen = NSScreen.screens.count > 1 ? NSScreen.screens[NSScreen.screens.count - 1] : NSScreen.screens[0]
let start = screen.visibleFrame.insetBy(dx: screen.visibleFrame.width * 0.3, dy: screen.visibleFrame.height * 0.3)
let window = NSWindow(contentRect: start, styleMask: [.titled, .closable, .resizable, .fullSizeContentView], backing: .buffered, defer: false, screen: screen)
window.setFrame(start, display: false)
window.title = "Test KyberNow : plein écran"
window.collectionBehavior = [.fullScreenPrimary, .managed]
window.isReleasedWhenClosed = false
let parent = NSView(frame: NSRect(origin: .zero, size: start.size))
parent.wantsLayer = true
window.contentView = parent
let surface = MetalSurface(frame: parent.bounds)
surface.autoresizingMask = [.width, .height]
parent.addSubview(surface)

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

let renderer = VideoRenderer()!
var rendered = 0, presented = 0, redrawn = 0, discarded = 0
renderer.onPresentStats = { s in
    rendered += s.rendered; presented += s.presented; redrawn += s.redrawn; discarded += s.discarded
}
// Vue occultée : le rendu attend, comme SessionEngine.setScreenOccluded.
renderer.paused = true
surface.show(renderer)
window.orderFrontRegardless()
let decoder = VideoDecoder()
decoder.setCodec(0x68323635)
var format = "?", frameSize = "?"
decoder.onFrame = { frame in
    let f = CVPixelBufferGetPixelFormatType(frame.buffer)
    format = String(bytes: [24, 16, 8, 0].map { UInt8((f >> $0) & 0xFF) }, encoding: .ascii) ?? "?"
    frameSize = "\(CVPixelBufferGetWidth(frame.buffer))×\(CVPixelBufferGetHeight(frame.buffer))"
    renderer.submit(frame)
}

/// Part des pixels non noirs d'une image.
func litRatio(_ image: CGImage) -> Double? {
    let w = image.width, h = image.height
    var pixels = [UInt8](repeating: 0, count: w * h * 4)
    guard let ctx = CGContext(data: &pixels, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                              space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
    ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
    var lit = 0
    for i in stride(from: 0, to: pixels.count, by: 4) where Int(pixels[i]) + Int(pixels[i + 1]) + Int(pixels[i + 2]) > 24 { lit += 1 }
    return Double(lit) / Double(w * h)
}
for (i, au) in aus.enumerated() {
    au.withUnsafeBufferPointer { decoder.decode($0, isConfig: false, isKey: i == 0, pts: UInt64(i)) }
}
print("\(aus.count) images décodées pendant que la fenêtre est occultée, sur l'écran \(screen.localizedName)")

func fail(_ why: String) -> Never {
    print("ÉCHEC \(why)")
    exit(1)
}

var entered = false
NotificationCenter.default.addObserver(forName: NSWindow.didEnterFullScreenNotification, object: window, queue: .main) { _ in
    entered = true
    // Fin du passage : comme SessionController.fullScreenEntered.
    renderer.redraw()
}
DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
    // La fenêtre redevient visible pendant son passage en plein écran : le rendu reprend.
    app.activate(ignoringOtherApps: true)
    window.toggleFullScreen(nil)
    renderer.paused = false
}
DispatchQueue.main.asyncAfter(deadline: .now() + 4.5) {
    guard entered else { fail("la fenêtre n'est pas passée en plein écran") }
    renderer.reportPresentationNow()
    let scale = window.backingScaleFactor
    let want = CGSize(width: window.frame.width * scale, height: window.frame.height * scale)
    let have = renderer.layer.drawableSize
    print("plein écran \(Int(window.frame.width))×\(Int(window.frame.height)) points, image \(Int(have.width))×\(Int(have.height)) pixels (attendu \(Int(want.width))×\(Int(want.height)))")
    print("\(rendered) rendue(s), \(presented) montrée(s), \(discarded) écartée(s), \(redrawn) redessinée(s) montrée(s)")
    window.toggleFullScreen(nil)
    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
        decoder.invalidate()
        guard abs(have.width - want.width) < 2, abs(have.height - want.height) < 2 else { fail("l'image n'a pas la taille du plein écran") }
        guard rendered >= 1 else { fail("l'image arrivée pendant la pause n'a pas été rendue à la reprise") }
        guard redrawn >= 1 else { fail("aucune image redessinée montrée après le passage en plein écran") }
        // L'image rendue est entière : presque aucun pixel noir (la mire couvre toute l'image).
        guard let shot = renderer.snapshot(maxWidth: 800), let lit = litRatio(shot) else { fail("image rendue illisible") }
        print("image \(format) \(frameSize) : \(Int(lit * 100)) % de pixels non noirs")
        guard lit > 0.9 else { fail("image rendue incomplète") }
        print("ok    l'image arrivée pendant la pause est rendue, puis redessinée et montrée à la taille du plein écran")
        exit(0)
    }
}
app.run()
