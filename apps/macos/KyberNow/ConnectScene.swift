// Scène de connexion et de fin de session (écrans « 5 · Connexion » et
// « 7 · Déconnexion » de la maquette), en Core Animation : tout est animé par
// le serveur d'affichage de macOS, sur le GPU, sans solliciter l'app pendant
// que le décodeur vidéo démarre.
//
// Coordonnées de la maquette (1280 × 800) ramenées à la fenêtre : le logo est
// au centre, 40 points au-dessus du milieu ; les ondes le traversent.

import AppKit
import QuartzCore
import SwiftUI

final class ConnectScene: NSView {
    private let backdrop = CALayer()
    private let waves = CALayer()
    private let halo = CAGradientLayer()
    private let mark = CALayer()
    private let effects = CALayer()

    private static let mint = NSColor(red: 0x6F / 255, green: 0xE3 / 255, blue: 0xC1 / 255, alpha: 1)
    private static let mintLight = NSColor(red: 0x9F / 255, green: 0xF0 / 255, blue: 0xD8 / 255, alpha: 1)
    private static let background = NSColor(red: 0x0D / 255, green: 0x11 / 255, blue: 0x17 / 255, alpha: 1)
    /// Courbe d'éjection des particules de la maquette : cubic-bezier(.1,.8,.22,1).
    private static let burstCurve = CAMediaTimingFunction(controlPoints: 0.1, 0.8, 0.22, 1)

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        guard let root = layer else { return }
        backdrop.backgroundColor = Self.background.cgColor
        root.addSublayer(backdrop)
        root.addSublayer(waves)
        halo.type = .radial
        halo.colors = [Self.mint.withAlphaComponent(0.22).cgColor, Self.mint.withAlphaComponent(0).cgColor]
        halo.locations = [0, 0.68]
        halo.startPoint = CGPoint(x: 0.5, y: 0.5)
        halo.endPoint = CGPoint(x: 1, y: 1)
        root.addSublayer(halo)
        mark.contentsGravity = .resizeAspect
        root.addSublayer(mark)
        root.addSublayer(effects)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    override var isFlipped: Bool { true }
    // Décor : ne capte jamais la souris.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// Centre du logo.
    private var center: CGPoint { CGPoint(x: bounds.midX, y: bounds.midY - 40) }

    override func layout() {
        super.layout()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        backdrop.frame = bounds
        waves.frame = bounds
        effects.frame = bounds
        halo.bounds = CGRect(x: 0, y: 0, width: 240, height: 240)
        halo.position = center
        mark.bounds = CGRect(x: 0, y: 0, width: 120, height: 120)
        mark.position = center
        if mark.contents == nil { mark.contents = Self.markImage() }
        buildWaves()
        CATransaction.commit()
    }

    private static func markImage() -> CGImage? {
        let renderer = ImageRenderer(content: Mark(size: 120, animated: false))
        renderer.scale = 3
        return renderer.cgImage
    }

    // MARK: - Attente

    /// Les trois ondes de la maquette, ramenées à la largeur de la fenêtre.
    private func buildWaves() {
        waves.sublayers?.forEach { $0.removeFromSuperlayer() }
        let sx = bounds.width / 1280
        let c = center
        func path(_ dy: CGFloat) -> CGPath {
            let p = CGMutablePath()
            func pt(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: x * sx, y: c.y + (y - 360 + dy) * min(1, sx * 1.4)) }
            p.move(to: pt(-20, 374))
            p.addLine(to: pt(560, 374))
            p.addCurve(to: pt(618, 404), control1: pt(586, 374), control2: pt(596, 404))
            p.addCurve(to: pt(676, 306), control1: pt(644, 404), control2: pt(650, 306))
            p.addCurve(to: pt(730, 374), control1: pt(700, 306), control2: pt(706, 374))
            p.addLine(to: pt(1300, 374))
            return p
        }
        let length = bounds.width * 1.12
        let faint = CAShapeLayer()
        faint.path = path(0)
        faint.fillColor = nil
        faint.strokeColor = Self.mint.withAlphaComponent(0.07).cgColor
        faint.lineWidth = 2.5
        waves.addSublayer(faint)
        for (dy, alpha, offset) in [(0.0, 0.75, 0.0), (12.0, 0.3, 1.1), (-12.0, 0.3, 2.2)] {
            let wave = CAShapeLayer()
            wave.path = path(dy)
            wave.fillColor = nil
            wave.strokeColor = Self.mint.withAlphaComponent(alpha).cgColor
            wave.lineWidth = 2.5
            wave.lineCap = .round
            wave.lineDashPattern = [NSNumber(value: Double(length * 0.17)), NSNumber(value: Double(length * 0.83))]
            let flow = CABasicAnimation(keyPath: "lineDashPhase")
            flow.fromValue = length
            flow.toValue = 0
            flow.duration = 3.2
            flow.repeatCount = .infinity
            flow.timeOffset = offset
            wave.add(flow, forKey: "flow")
            waves.addSublayer(wave)
        }
        startBreathing()
    }

    private func startBreathing() {
        let breathe = CABasicAnimation(keyPath: "transform.scale")
        breathe.fromValue = 1
        breathe.toValue = 1.035
        breathe.duration = 1.3
        breathe.autoreverses = true
        breathe.repeatCount = .infinity
        breathe.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        mark.add(breathe, forKey: "breathe")

        let glow = CAAnimationGroup()
        let o = CABasicAnimation(keyPath: "opacity")
        o.fromValue = 0.25
        o.toValue = 0.55
        let s = CABasicAnimation(keyPath: "transform.scale")
        s.fromValue = 0.92
        s.toValue = 1.08
        glow.animations = [o, s]
        glow.duration = 1.3
        glow.autoreverses = true
        glow.repeatCount = .infinity
        glow.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        halo.add(glow, forKey: "glow")
    }

    // MARK: - Apparition

    /// Gerbe de particules, puis l'image de l'ordinateur apparaît en fondu.
    func reveal(_ video: NSView) {
        let now = CACurrentMediaTime()
        let c = center
        CATransaction.begin()

        // Le logo éclate.
        mark.removeAnimation(forKey: "breathe")
        let pop = CAKeyframeAnimation(keyPath: "transform.scale")
        pop.values = [1, 1.12, 1.5]
        pop.keyTimes = [0, 0.35, 1]
        let popFade = CAKeyframeAnimation(keyPath: "opacity")
        popFade.values = [1, 1, 0]
        popFade.keyTimes = [0, 0.35, 1]
        add(group([pop, popFade], duration: 0.55, curve: CAMediaTimingFunction(controlPoints: 0.3, 0.6, 0.2, 1)), to: mark, final: 0)
        halo.removeAllAnimations()
        halo.opacity = 0

        // Éclat central.
        let flash = CAGradientLayer()
        flash.type = .radial
        flash.colors = [NSColor(red: 220 / 255, green: 1, blue: 244 / 255, alpha: 0.9).cgColor, Self.mint.withAlphaComponent(0.25).cgColor, Self.mint.withAlphaComponent(0).cgColor]
        flash.locations = [0, 0.35, 0.7]
        flash.startPoint = CGPoint(x: 0.5, y: 0.5)
        flash.endPoint = CGPoint(x: 1, y: 1)
        flash.bounds = CGRect(x: 0, y: 0, width: 600, height: 600)
        flash.position = c
        flash.opacity = 0
        effects.addSublayer(flash)
        let flashFade = CAKeyframeAnimation(keyPath: "opacity")
        flashFade.values = [0, 0.5, 0]
        flashFade.keyTimes = [0, 0.25, 1]
        add(group([flashFade], duration: 0.9, curve: CAMediaTimingFunction(name: .easeOut)), to: flash, final: 0)

        // Deux anneaux qui s'ouvrent.
        for (delay, duration, width, color) in [(0.0, 1.15, 2.0, Self.mintLight), (0.12, 1.4, 1.0, Self.mint.withAlphaComponent(0.6))] {
            let ring = CAShapeLayer()
            ring.path = CGPath(ellipseIn: CGRect(x: -60, y: -60, width: 120, height: 120), transform: nil)
            ring.fillColor = nil
            ring.strokeColor = color.cgColor
            ring.lineWidth = width
            ring.position = c
            ring.opacity = 0
            effects.addSublayer(ring)
            let grow = CABasicAnimation(keyPath: "transform.scale")
            grow.fromValue = 0.35
            grow.toValue = 7
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 0.9
            fade.toValue = 0
            add(group([grow, fade], duration: duration, curve: CAMediaTimingFunction(controlPoints: 0.15, 0.75, 0.25, 1), begin: now + delay), to: ring, final: 0)
        }

        // 64 particules sur quatre portées, comme la maquette.
        let colors = [Self.mint, Self.mintLight, NSColor(white: 0.97, alpha: 1), Self.mint, NSColor(red: 0.79, green: 0.98, blue: 0.91, alpha: 1)]
        let kinds: [(distance: CGFloat, duration: Double)] = [(200, 1.3), (320, 1.55), (450, 1.8), (320, 1.55), (640, 2.3), (200, 1.3), (450, 1.8)]
        let reach = max(1, min(bounds.width / 1280, 1.6))
        for i in 0..<64 {
            func r(_ k: Double) -> Double {
                let x = sin(Double(i) * 12.9898 + k * 78.233) * 43758.5453
                return x - x.rounded(.down)
            }
            let angle = (Double(i) / 64 * 360 + (r(1) - 0.5) * 18) * .pi / 180
            let size = CGFloat(3 + (r(2) * 5).rounded())
            let kind = kinds[i % kinds.count]
            let color = colors[i % colors.count]
            let dot = CALayer()
            dot.bounds = CGRect(x: 0, y: 0, width: size, height: size)
            dot.cornerRadius = size / 2
            dot.backgroundColor = color.cgColor
            if size > 5 {
                dot.shadowColor = color.cgColor
                dot.shadowOpacity = 1
                dot.shadowRadius = 5
                dot.shadowOffset = .zero
            }
            dot.position = c
            dot.opacity = 0
            effects.addSublayer(dot)
            let target = CGPoint(x: c.x + cos(angle) * kind.distance * reach, y: c.y + sin(angle) * kind.distance * reach)
            let move = CABasicAnimation(keyPath: "position")
            move.fromValue = c
            move.toValue = target
            let shrink = CABasicAnimation(keyPath: "transform.scale")
            shrink.fromValue = 0.4 * 2.5
            shrink.toValue = 0.15 * 2.5
            let fade = CAKeyframeAnimation(keyPath: "opacity")
            fade.values = [0, 1, 0]
            fade.keyTimes = [0, 0.06, 1]
            add(group([move, shrink, fade], duration: kind.duration, curve: Self.burstCurve, begin: now + r(3) * 0.14), to: dot, final: 0)
        }

        // Le décor s'efface, l'image arrive en fondu avec un léger zoom.
        for layer in [backdrop, waves] {
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 1
            fade.toValue = 0
            add(group([fade], duration: 1.1, curve: CAMediaTimingFunction(name: .easeOut), begin: now + 0.25), to: layer, final: 0)
        }
        if let videoLayer = video.layer {
            let size = video.bounds.size
            let zoom = CATransform3DConcat(
                CATransform3DConcat(CATransform3DMakeTranslation(-size.width / 2, -size.height / 2, 0), CATransform3DMakeScale(1.06, 1.06, 1)),
                CATransform3DMakeTranslation(size.width / 2, size.height / 2, 0)
            )
            let scale = CABasicAnimation(keyPath: "transform")
            scale.fromValue = zoom
            scale.toValue = CATransform3DIdentity
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 0
            fade.toValue = 1
            add(group([scale, fade], duration: 1.5, curve: CAMediaTimingFunction(controlPoints: 0.2, 0.7, 0.2, 1), begin: now + 0.25), to: videoLayer, final: 1)
        }

        CATransaction.setCompletionBlock { [weak self] in self?.effects.sublayers?.forEach { $0.removeFromSuperlayer() } }
        CATransaction.commit()
    }

    /// Avant l'apparition, l'image reste invisible sous le décor.
    func hide(_ video: NSView) {
        video.layer?.opacity = 0
    }

    // MARK: - Fin de session

    /// L'image se referme, le décor et le logo reviennent.
    func close(_ video: NSView) {
        let now = CACurrentMediaTime()
        CATransaction.begin()
        if let videoLayer = video.layer {
            let size = video.bounds.size
            let shrink = CATransform3DConcat(
                CATransform3DConcat(CATransform3DMakeTranslation(-size.width / 2, -size.height / 2, 0), CATransform3DMakeScale(0.82, 0.82, 1)),
                CATransform3DMakeTranslation(size.width / 2, size.height / 2, 0)
            )
            let scale = CABasicAnimation(keyPath: "transform")
            scale.fromValue = CATransform3DIdentity
            scale.toValue = shrink
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 1
            fade.toValue = 0
            add(group([scale, fade], duration: 0.9, curve: CAMediaTimingFunction(controlPoints: 0.4, 0, 0.2, 1)), to: videoLayer, final: 0)
        }
        for layer in [backdrop, waves] {
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 0
            fade.toValue = 1
            add(group([fade], duration: 0.8, curve: CAMediaTimingFunction(name: .easeOut), begin: now + 0.2), to: layer, final: 1)
        }
        let grow = CABasicAnimation(keyPath: "transform.scale")
        grow.fromValue = 0.9
        grow.toValue = 1
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.fromValue = 0
        fade.toValue = 1
        add(group([grow, fade], duration: 0.6, curve: CAMediaTimingFunction(controlPoints: 0.2, 0.7, 0.2, 1), begin: now + 0.7), to: mark, final: 1)
        CATransaction.commit()
    }

    // MARK: - Outils

    private func group(_ animations: [CAAnimation], duration: CFTimeInterval, curve: CAMediaTimingFunction, begin: CFTimeInterval? = nil) -> CAAnimationGroup {
        let g = CAAnimationGroup()
        g.animations = animations
        g.duration = duration
        g.timingFunction = curve
        if let begin { g.beginTime = begin }
        g.fillMode = .both
        return g
    }

    /// Ajoute l'animation et fixe la valeur finale d'opacité du calque.
    private func add(_ animation: CAAnimationGroup, to layer: CALayer, final opacity: Float) {
        layer.opacity = opacity
        layer.add(animation, forKey: nil)
    }
}
