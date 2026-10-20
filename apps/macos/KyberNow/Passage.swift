// Le passage (maquette « App · Le passage ») : l'accueil, photographié, se
// coupe en deux le long de la couture à 45° ; un tube néon s'allume dans la
// coupure pendant la préparation, puis les deux moitiés s'écartent et
// l'image de l'ordinateur apparaît derrière. À la fin, elles se referment sur
// la dernière image avant que l'accueil revienne.
//
// Core Animation : tout est animé par le serveur d'affichage, sur le GPU,
// sans solliciter l'app pendant que le décodeur démarre.

import AppKit
import QuartzCore

final class PassageScene: NSView {
    private let freeze = CALayer()
    private let halves = [CALayer(), CALayer()]
    private let masks = [CAShapeLayer(), CAShapeLayer()]
    private let band = CAGradientLayer()
    private let tube = CALayer()
    private let tubeHalo = CALayer()
    private let tubeSheath = CALayer()
    private let tubeCore = CALayer()
    private let tubeFade = CAGradientLayer()
    private var reduceMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }

    private static let signal = NSColor(red: 0x9A / 255, green: 0x8C / 255, blue: 0xFF / 255, alpha: 1)
    private static let signalFill = NSColor(red: 0x6D / 255, green: 0x5B / 255, blue: 0xFF / 255, alpha: 1)
    private static let core = NSColor(red: 0xF4 / 255, green: 0xF1 / 255, blue: 0xFF / 255, alpha: 1)
    /// Sortie de la charte : cubic-bezier(.2, .9, .1, 1).
    private static let out = CAMediaTimingFunction(controlPoints: 0.2, 0.9, 0.1, 1)

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        guard let root = layer else { return }
        freeze.contentsGravity = .resizeAspect
        freeze.backgroundColor = NSColor.black.cgColor
        freeze.opacity = 0
        root.addSublayer(freeze)
        for (half, mask) in zip(halves, masks) {
            half.contentsGravity = .resizeAspectFill
            half.backgroundColor = NSColor(red: 0x05 / 255, green: 0x07 / 255, blue: 0x0A / 255, alpha: 1).cgColor
            half.mask = mask
            root.addSublayer(half)
        }
        band.type = .radial
        band.colors = [Self.signalFill.withAlphaComponent(0.42).cgColor, Self.signalFill.withAlphaComponent(0).cgColor]
        band.startPoint = CGPoint(x: 0.5, y: 0.5)
        band.endPoint = CGPoint(x: 1, y: 1)
        band.opacity = 0
        root.addSublayer(band)

        // Tube : halo large, gaine Spectre, cœur presque blanc ; bouts en fondu.
        for (layer, color, shadow) in [(tubeHalo, Self.signalFill.withAlphaComponent(0.55), 26.0), (tubeSheath, Self.signal, 6.0), (tubeCore, Self.core, 2.0)] {
            layer.backgroundColor = color.cgColor
            layer.shadowColor = (layer === tubeCore ? NSColor.white : Self.signal).cgColor
            layer.shadowOpacity = 1
            layer.shadowRadius = shadow
            layer.shadowOffset = .zero
            tube.addSublayer(layer)
        }
        tubeFade.startPoint = CGPoint(x: 0, y: 0.5)
        tubeFade.endPoint = CGPoint(x: 1, y: 0.5)
        tubeFade.colors = [NSColor.clear.cgColor, NSColor.white.cgColor, NSColor.white.cgColor, NSColor.clear.cgColor]
        tubeFade.locations = [0, 0.14, 0.86, 1]
        tube.mask = tubeFade
        tube.opacity = 0
        root.addSublayer(tube)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    override var isFlipped: Bool { true }
    // Décor : ne capte jamais la souris.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// Point haut de la couture (elle descend à 45° vers la gauche).
    private var seamTop: CGFloat { (bounds.width + bounds.height) / 2 }

    override func layout() {
        super.layout()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        let w = bounds.width, h = bounds.height, c = seamTop
        freeze.frame = bounds
        for (i, half) in halves.enumerated() {
            half.bounds = bounds
            half.position = CGPoint(x: w / 2, y: h / 2)
            masks[i].frame = bounds
            let path = CGMutablePath()
            if i == 0 {
                path.addLines(between: [.zero, CGPoint(x: c, y: 0), CGPoint(x: c - h, y: h), CGPoint(x: 0, y: h)])
            } else {
                path.addLines(between: [CGPoint(x: c, y: 0), CGPoint(x: w, y: 0), CGPoint(x: w, y: h), CGPoint(x: c - h, y: h)])
            }
            path.closeSubpath()
            masks[i].path = path
        }
        let length = h * 2.squareRoot()
        let rotation = CATransform3DMakeRotation(-.pi / 4, 0, 0, 1)
        band.bounds = CGRect(x: 0, y: 0, width: length * 1.4, height: 300)
        band.position = CGPoint(x: c - h / 2, y: h / 2)
        band.transform = rotation
        tube.bounds = CGRect(x: 0, y: 0, width: length, height: 60)
        tube.position = CGPoint(x: c - h / 2, y: h / 2)
        tube.transform = rotation
        tubeFade.frame = tube.bounds
        tubeHalo.frame = CGRect(x: 0, y: 30 - 8, width: length, height: 16)
        tubeHalo.cornerRadius = 8
        tubeSheath.frame = CGRect(x: 0, y: 30 - 3, width: length, height: 6)
        tubeSheath.cornerRadius = 3
        tubeCore.frame = CGRect(x: 0, y: 30 - 1.5, width: length, height: 3)
        tubeCore.cornerRadius = 1.5
        CATransaction.commit()
    }

    // MARK: - Déroulé

    /// Avant tout : l'accueil tel qu'il était, en deux moitiés jointes (rien ne bouge à l'écran).
    func prepare(home: CGImage?) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        for half in halves {
            half.removeAllAnimations()
            half.contents = home
            half.opacity = 1
            half.transform = CATransform3DIdentity
        }
        freeze.opacity = 0
        tube.opacity = 0
        band.opacity = 0
        CATransaction.commit()
    }

    /// Le tube s'allume dans la couture (grésillement), puis respire pendant l'attente.
    func ignite() {
        tube.removeAllAnimations()
        band.removeAllAnimations()
        tube.opacity = 1
        band.opacity = 0.75
        guard !reduceMotion else { return }
        let flicker = CAKeyframeAnimation(keyPath: "opacity")
        flicker.values = [0, 0.9, 0.1, 0.95, 0.05, 1, 0.3, 1]
        flicker.keyTimes = [0, 0.06, 0.1, 0.16, 0.22, 0.3, 0.34, 0.44]
        flicker.duration = 1
        let grow = CABasicAnimation(keyPath: "transform.scale.x")
        grow.fromValue = 0
        grow.toValue = 1
        grow.duration = 0.45
        grow.timingFunction = Self.out
        let breathe = CABasicAnimation(keyPath: "opacity")
        breathe.fromValue = 1
        breathe.toValue = 0.7
        breathe.duration = 1.6
        breathe.autoreverses = true
        breathe.repeatCount = .infinity
        breathe.beginTime = CACurrentMediaTime() + 1
        tube.add(flicker, forKey: "allumage")
        tube.add(grow, forKey: "etirement")
        tube.add(breathe, forKey: "respiration")
        let fadeIn = CABasicAnimation(keyPath: "opacity")
        fadeIn.fromValue = 0
        fadeIn.duration = 0.6
        band.add(fadeIn, forKey: "apparition")
    }

    /// L'image est prête : les moitiés s'écartent, le néon s'éteint.
    func open() {
        let d = max(bounds.width, bounds.height) * 0.18
        let duration: CFTimeInterval = reduceMotion ? 0.25 : 1.1
        for (i, half) in halves.enumerated() {
            let dir: CGFloat = i == 0 ? -1 : 1
            half.removeAllAnimations()
            let move = CABasicAnimation(keyPath: "transform")
            move.fromValue = CATransform3DIdentity
            move.toValue = reduceMotion ? CATransform3DIdentity : CATransform3DMakeTranslation(dir * d, dir * d, 0)
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 1
            fade.toValue = 0
            let group = CAAnimationGroup()
            group.animations = [move, fade]
            group.duration = duration
            group.timingFunction = Self.out
            half.opacity = 0
            half.transform = move.toValue as! CATransform3D
            half.add(group, forKey: "ouverture")
        }
        fadeOut(tube, after: reduceMotion ? 0 : 0.45, duration: 0.6)
        fadeOut(band, after: reduceMotion ? 0 : 0.2, duration: 0.7)
    }

    /// Fin : la dernière image se fige, les moitiés de l'accueil reviennent et se joignent.
    func close(over last: CGImage?, home: CGImage?, completion: @escaping () -> Void) {
        let d = max(bounds.width, bounds.height) * 0.18
        let duration: CFTimeInterval = reduceMotion ? 0.2 : 0.6
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        freeze.contents = last
        freeze.opacity = last == nil ? 0 : 1
        CATransaction.commit()
        CATransaction.begin()
        CATransaction.setCompletionBlock(completion)
        for (i, half) in halves.enumerated() {
            let dir: CGFloat = i == 0 ? -1 : 1
            half.removeAllAnimations()
            if let home { half.contents = home }
            let move = CABasicAnimation(keyPath: "transform")
            move.fromValue = reduceMotion ? CATransform3DIdentity : CATransform3DMakeTranslation(dir * d, dir * d, 0)
            move.toValue = CATransform3DIdentity
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 0
            fade.toValue = 1
            let group = CAAnimationGroup()
            group.animations = [move, fade]
            group.duration = duration
            group.timingFunction = Self.out
            half.opacity = 1
            half.transform = CATransform3DIdentity
            half.add(group, forKey: "fermeture")
        }
        // Un dernier éclair dans la couture au moment où elles se rejoignent.
        tube.removeAllAnimations()
        band.removeAllAnimations()
        let flash = CAKeyframeAnimation(keyPath: "opacity")
        flash.values = [0, 0, 1, 0]
        flash.keyTimes = [0, 0.55, 0.7, 1]
        flash.duration = duration + 0.15
        tube.opacity = 0
        tube.add(flash, forKey: "eclair")
        CATransaction.commit()
    }

    /// Échec : le tube s'éteint, l'accueil reste joint.
    func dim() {
        fadeOut(tube, after: 0, duration: 0.4)
        fadeOut(band, after: 0, duration: 0.4)
    }

    private func fadeOut(_ layer: CALayer, after delay: CFTimeInterval, duration: CFTimeInterval) {
        let current = layer.presentation()?.opacity ?? layer.opacity
        layer.removeAllAnimations()
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.fromValue = current
        fade.toValue = 0
        fade.beginTime = CACurrentMediaTime() + delay
        fade.duration = duration
        fade.fillMode = .backwards
        layer.opacity = 0
        layer.add(fade, forKey: "extinction")
    }
}

/// Délégué de fenêtre posé le temps d'une session dans la fenêtre de
/// l'accueil : il cache la barre des menus et le Dock en plein écran et
/// prévient de la fermeture, et transmet tout le reste au délégué de SwiftUI.
final class SessionWindowDelegate: NSObject, NSWindowDelegate {
    weak var original: NSWindowDelegate?
    var onClose: (() -> Void)?
    var onExitFullScreen: (() -> Void)?

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (original?.responds(to: selector) ?? false)
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        original?.responds(to: selector) == true ? original : nil
    }

    // Plein écran sans barre des menus ni Dock : le haut de l'écran est à la barre KyberNow.
    func window(_ window: NSWindow, willUseFullScreenPresentationOptions proposedOptions: NSApplication.PresentationOptions = []) -> NSApplication.PresentationOptions {
        [.fullScreen, .hideDock, .hideMenuBar]
    }

    func windowWillClose(_ notification: Notification) {
        onClose?()
        original?.windowWillClose?(notification)
    }

    func windowDidExitFullScreen(_ notification: Notification) {
        onExitFullScreen?()
        original?.windowDidExitFullScreen?(notification)
    }
}
