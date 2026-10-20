// Vue qui porte l'image dessinée par Metal (VideoRenderer).
//
// La couche Metal est la couche de la vue elle-même (posée avant
// `wantsLayer`, comme le veut AppKit pour une vue qui héberge sa couche) :
// opaque, plein cadre, sans sous-couche ni transformation. En plein écran,
// rien n'est composé par-dessus quand la barre et les panneaux sont cachés :
// l'image peut aller directement à l'écran (à vérifier avec
// MTL_HUD_ENABLED=1). Chaque rendu (un par flux) dessine dans cette couche
// (`VideoRenderer.attach(to:)`) ; remplacer la couche d'une vue déjà gérée
// par AppKit n'est pas fiable, la couche reste donc la même d'un flux à
// l'autre.

import AppKit
import QuartzCore

final class MetalSurface: NSView {
    private let metal = CAMetalLayer()
    private var renderer: VideoRenderer?

    override init(frame: NSRect) {
        super.init(frame: frame)
        metal.backgroundColor = CGColor(gray: 0, alpha: 1)
        metal.isOpaque = true
        layer = metal
        wantsLayer = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    override var isOpaque: Bool { true }

    /// Branche le rendu (nil : VLC dessine, la surface se cache).
    func show(_ renderer: VideoRenderer?) {
        // L'ancien rendu (flux précédent, encore en vie quelques instants) retrouve une couche à lui.
        self.renderer?.attach(to: nil)
        self.renderer = renderer
        isHidden = renderer == nil
        guard let renderer else { return }
        renderer.attach(to: metal)
        updateDrawableSize()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
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
        metal.contentsScale = scale
        renderer.setDrawableSize(CGSize(width: bounds.width * scale, height: bounds.height * scale))
    }
}
