// Vue qui porte l'image dessinée par Metal (VideoRenderer), version UIKit
// (Apple TV, iPhone et iPad).
//
// La couche Metal est une sous-couche créée par la vue elle-même, une fois
// dans une fenêtre, et le rendu ne s'y branche qu'à ce moment-là
// (`VideoRenderer.attach(to:)`, comme sur le Mac). Une couche Metal qui a
// présenté des images avant d'être posée dans une fenêtre reste hors écran
// pour toujours sur iOS : toutes ses images sont écartées (`presentedTime`
// à 0), jusqu'à un passage en arrière-plan (essai du 2026-10-22, iPhone 15
// Pro Max). Le rendu, qui démarre sur le fil réseau dès la première image
// décodée, dessinait dans sa propre couche, attachée un peu plus tard sur le
// fil principal : l'écran restait noir. Si l'écran écarte quand même tout
// pendant deux secondes, la couche est refaite (`heal`).

import QuartzCore
import UIKit

final class MetalSurface: UIView {
    private var metal: CAMetalLayer?
    private var renderer: VideoRenderer?

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        isUserInteractionEnabled = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    /// Branche le rendu (nil : rien à dessiner).
    func show(_ renderer: VideoRenderer?) {
        // L'ancien rendu (flux précédent, encore en vie quelques instants) retrouve une couche à lui.
        self.renderer?.attach(to: nil)
        self.renderer = renderer
        attachIfPossible()
    }

    /// L'écran écarte toutes les images : couche Metal neuve.
    func heal() {
        guard renderer != nil, window != nil else { return }
        KyberLog.video.notice("affichage bloqué : couche Metal refaite")
        metal?.removeFromSuperlayer()
        metal = nil
        attachIfPossible()
    }

    private func attachIfPossible() {
        guard let renderer, window != nil else { return }
        let layer = metal ?? makeLayer()
        renderer.attach(to: layer)
        updateDrawableSize()
    }

    private func makeLayer() -> CAMetalLayer {
        let layer = CAMetalLayer()
        layer.backgroundColor = CGColor(gray: 0, alpha: 1)
        layer.isOpaque = true
        layer.frame = self.layer.bounds
        self.layer.addSublayer(layer)
        metal = layer
        return layer
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        metal?.frame = layer.bounds
        CATransaction.commit()
        updateDrawableSize()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if metal == nil { attachIfPossible() } else { updateDrawableSize() }
    }

    private func updateDrawableSize() {
        guard let renderer, let metal else { return }
        let scale = window?.screen.nativeScale ?? UIScreen.main.nativeScale
        metal.contentsScale = scale
        renderer.setDrawableSize(CGSize(width: bounds.width * scale, height: bounds.height * scale))
    }
}
