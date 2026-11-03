// Icône de la barre des menus : le symbole de KyberNow Station (identité v2,
// brand/v2/mark), dessiné à la taille de la barre.
//  - en marche : image modèle (macOS la teinte selon le fond), la moitié
//    « signal » pleine, l'autre plus pâle ;
//  - session en cours : la moitié signal en violet (#9A8CFF), comme le logo ;
//  - arrêtée ou accord manquant : les deux moitiés au trait, pâles.

import AppKit

enum IconLook { case running, session, idle }

private func halves() -> (NSBezierPath, NSBezierPath) {
    // Grille de 48 du symbole (y vers le bas, vue retournée).
    let top = NSBezierPath()
    top.move(to: NSPoint(x: 7, y: 0))
    top.line(to: NSPoint(x: 43.1, y: 0))
    top.line(to: NSPoint(x: 0, y: 43.1))
    top.line(to: NSPoint(x: 0, y: 7))
    top.appendArc(withCenter: NSPoint(x: 7, y: 7), radius: 7, startAngle: 180, endAngle: 270)
    top.close()
    let bottom = NSBezierPath()
    bottom.move(to: NSPoint(x: 48, y: 4.9))
    bottom.line(to: NSPoint(x: 48, y: 41))
    bottom.appendArc(withCenter: NSPoint(x: 41, y: 41), radius: 7, startAngle: 0, endAngle: 90)
    bottom.line(to: NSPoint(x: 4.9, y: 48))
    bottom.close()
    return (top, bottom)
}

func statusIcon(_ look: IconLook) -> NSImage {
    let size = NSSize(width: 16, height: 16)
    let image = NSImage(size: size, flipped: true) { rect in
        let scale = rect.width / 48
        let transform = AffineTransform(scale: scale)
        let (top, bottom) = halves()
        top.transform(using: transform)
        bottom.transform(using: transform)
        switch look {
        case .running:
            NSColor.black.setFill()
            top.fill()
            NSColor.black.withAlphaComponent(0.5).setFill()
            bottom.fill()
        case .session:
            NSColor(srgbRed: 0x9A / 255, green: 0x8C / 255, blue: 1, alpha: 1).setFill()
            top.fill()
            NSColor.labelColor.setFill()
            bottom.fill()
        case .idle:
            NSColor.black.withAlphaComponent(0.55).setStroke()
            for p in [top, bottom] {
                p.lineWidth = 1.1
                p.stroke()
            }
        }
        return true
    }
    // La version en couleur suit le thème par `labelColor` ; les autres sont des images modèles.
    image.isTemplate = look != .session
    return image
}
