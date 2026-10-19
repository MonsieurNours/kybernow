// Gerbe de particules de l'apparition, en Core Animation : particules,
// anneaux et éclat sont animés par le serveur d'affichage de macOS, sur le
// GPU, sans solliciter l'app au moment où le décodeur vidéo démarre.

import AppKit
import QuartzCore

final class BurstView: NSView {
    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.masksToBounds = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    // Purement décoratif : ne capte jamais la souris.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    private static let mint = NSColor(red: 0x6F / 255, green: 0xE3 / 255, blue: 0xC1 / 255, alpha: 1)

    /// Petit disque doux, texture des particules.
    private static let dot: CGImage? = {
        let size = 32
        guard let ctx = CGContext(data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        let colors = [CGColor(gray: 1, alpha: 1), CGColor(gray: 1, alpha: 0)] as CFArray
        guard let gradient = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors, locations: [0.35, 1]) else { return nil }
        let c = CGPoint(x: size / 2, y: size / 2)
        ctx.drawRadialGradient(gradient, startCenter: c, startRadius: 0, endCenter: c, endRadius: CGFloat(size) / 2, options: [])
        return ctx.makeImage()
    }()

    func play() {
        guard let root = layer else { return }
        let center = CGPoint(x: bounds.midX, y: bounds.midY + 20)
        CATransaction.begin()
        CATransaction.setCompletionBlock { [weak self] in self?.layer?.sublayers?.forEach { $0.removeFromSuperlayer() } }

        // Éclat central
        let flash = CAGradientLayer()
        flash.type = .radial
        flash.colors = [Self.mint.withAlphaComponent(0.5).cgColor, Self.mint.withAlphaComponent(0).cgColor]
        flash.startPoint = CGPoint(x: 0.5, y: 0.5)
        flash.endPoint = CGPoint(x: 1, y: 1)
        flash.bounds = CGRect(x: 0, y: 0, width: 620, height: 620)
        flash.position = center
        flash.opacity = 0
        root.addSublayer(flash)
        flash.add(fade(from: 1, to: 0, duration: 0.8), forKey: "flash")

        // Anneaux qui s'ouvrent
        for (delay, width, alpha) in [(0.0, 2.0, Float(0.85)), (0.12, 1.0, Float(0.55))] {
            let ring = CAShapeLayer()
            ring.path = CGPath(ellipseIn: CGRect(x: -60, y: -60, width: 120, height: 120), transform: nil)
            ring.fillColor = nil
            ring.strokeColor = Self.mint.cgColor
            ring.lineWidth = width
            ring.position = center
            ring.opacity = 0
            root.addSublayer(ring)
            let grow = CABasicAnimation(keyPath: "transform.scale")
            grow.fromValue = 0.6
            grow.toValue = 16
            let group = CAAnimationGroup()
            group.animations = [grow, fade(from: alpha, to: 0, duration: 1.2)]
            group.duration = 1.2
            group.beginTime = CACurrentMediaTime() + delay
            group.timingFunction = CAMediaTimingFunction(controlPoints: 0.12, 0.8, 0.22, 1)
            ring.add(group, forKey: "ring")
        }

        // Particules : une seule bouffée, puis l'émetteur se tait.
        let emitter = CAEmitterLayer()
        emitter.frame = bounds
        emitter.emitterPosition = center
        emitter.emitterShape = .point
        emitter.renderMode = .additive
        emitter.emitterCells = [Self.mint, NSColor(red: 0.62, green: 0.94, blue: 0.85, alpha: 1), .white].map { color in
            let cell = CAEmitterCell()
            cell.contents = Self.dot
            cell.color = color.cgColor
            cell.birthRate = 700
            cell.lifetime = 1.7
            cell.lifetimeRange = 0.5
            cell.velocity = 460
            cell.velocityRange = 300
            cell.emissionRange = .pi * 2
            cell.scale = 0.28
            cell.scaleRange = 0.16
            cell.scaleSpeed = -0.12
            cell.alphaSpeed = -0.55
            return cell
        }
        root.addSublayer(emitter)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.07) { emitter.birthRate = 0 }

        // Fin de l'ensemble : tout est retiré (voir le bloc de fin).
        let keepAlive = CALayer()
        root.addSublayer(keepAlive)
        keepAlive.add(fade(from: 0, to: 0, duration: 2.4), forKey: "lifetime")
        CATransaction.commit()
    }

    private func fade(from: Float, to: Float, duration: CFTimeInterval) -> CABasicAnimation {
        let a = CABasicAnimation(keyPath: "opacity")
        a.fromValue = from
        a.toValue = to
        a.duration = duration
        a.timingFunction = CAMediaTimingFunction(name: .easeOut)
        return a
    }
}
