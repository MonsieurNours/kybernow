// Icône d'app macOS (.icns) depuis un SVG de l'identité (brand/v2/app-icon) :
//   swift svg-to-icns.swift <icône.svg> <sortie.icns>
// NSImage lit le SVG ; iconutil assemble les tailles d'Apple.
import AppKit

let args = CommandLine.arguments
guard args.count == 3, let svg = NSImage(contentsOfFile: args[1]) else {
    FileHandle.standardError.write("usage : svg-to-icns.swift <icône.svg> <sortie.icns>\n".data(using: .utf8)!)
    exit(2)
}
let set = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("kybernow-\(getpid()).iconset")
try? FileManager.default.createDirectory(at: set, withIntermediateDirectories: true)
for base in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let px = base * scale
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8, samplesPerPixel: 4,
                                   hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        svg.draw(in: NSRect(x: 0, y: 0, width: px, height: px))
        NSGraphicsContext.restoreGraphicsState()
        let name = scale == 1 ? "icon_\(base)x\(base).png" : "icon_\(base)x\(base)@2x.png"
        try! rep.representation(using: .png, properties: [:])!.write(to: set.appendingPathComponent(name))
    }
}
let p = Process()
p.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
p.arguments = ["-c", "icns", set.path, "-o", args[2]]
try! p.run()
p.waitUntilExit()
try? FileManager.default.removeItem(at: set)
exit(p.terminationStatus)
