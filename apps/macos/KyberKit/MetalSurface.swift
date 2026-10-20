// Vue qui porte l'image dessinée par Metal (VideoRenderer).
//
// La vue héberge une couche à elle (posée avant `wantsLayer`, comme le veut
// AppKit) ; la couche Metal du rendu y est une sous-couche qui suit sa
// taille. Remplacer la couche d'une vue déjà gérée par AppKit (la vue de
// session l'est) n'est pas fiable.

import AppKit
import QuartzCore

final class MetalSurface: NSView {
    private var renderer: VideoRenderer?

    override init(frame: NSRect) {
        super.init(frame: frame)
        let host = CALayer()
        host.backgroundColor = CGColor(gray: 0, alpha: 1)
        layer = host
        wantsLayer = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    /// Branche le rendu (nil : VLC dessine, la surface se cache).
    func show(_ renderer: VideoRenderer?) {
        self.renderer?.layer.removeFromSuperlayer()
        self.renderer = renderer
        isHidden = renderer == nil
        guard let renderer, let host = layer else { return }
        renderer.layer.frame = host.bounds
        renderer.layer.autoresizingMask = [.layerWidthSizable, .layerHeightSizable]
        host.addSublayer(renderer.layer)
        updateDrawableSize()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        renderer?.layer.frame = layer?.bounds ?? .zero
        CATransaction.commit()
        updateDrawableSize()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        updateDrawableSize()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateDrawableSize()
    }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    private func updateDrawableSize() {
        guard let renderer else { return }
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        renderer.layer.contentsScale = scale
        renderer.setDrawableSize(CGSize(width: bounds.width * scale, height: bounds.height * scale))
    }
}
