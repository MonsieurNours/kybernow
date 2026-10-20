// Vue qui porte l'image dessinée par Metal (VideoRenderer), version UIKit.
// La couche Metal du rendu est une sous-couche qui suit la taille de la vue.

import QuartzCore
import UIKit

final class MetalSurface: UIView {
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
        self.renderer?.layer.removeFromSuperlayer()
        self.renderer = renderer
        guard let renderer else { return }
        renderer.layer.frame = layer.bounds
        layer.addSublayer(renderer.layer)
        updateDrawableSize()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        renderer?.layer.frame = layer.bounds
        CATransaction.commit()
        updateDrawableSize()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        updateDrawableSize()
    }

    private func updateDrawableSize() {
        guard let renderer else { return }
        let scale = window?.screen.nativeScale ?? UIScreen.main.nativeScale
        renderer.layer.contentsScale = scale
        renderer.setDrawableSize(CGSize(width: bounds.width * scale, height: bounds.height * scale))
    }
}
