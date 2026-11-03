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
//
// HDR : la surface dit à son rendu si l'écran de sa fenêtre est HDR
// (`HDRDisplay.isHDR`), et le redit quand la fenêtre change d'écran ou que
// les écrans changent ; le rendu change de chaîne (docs/hdr.md).

import AppKit
import QuartzCore

final class MetalSurface: NSView {
    private let metal = CAMetalLayer()
    private var renderer: VideoRenderer?
    private var screenObservers: [NSObjectProtocol] = []
    /// L'écran de la fenêtre est HDR (dernière valeur dite au rendu).
    private(set) var screenHDR = false
    /// L'écran de la fenêtre gagne ou perd le HDR (fil principal).
    var onScreenHDR: ((Bool) -> Void)?
    /// Relève la marge EDR du moment (couche HDR `linear` : la courbe du shader s'y cale).
    private var headroomTimer: Timer?

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

    /// Branche le rendu (nil : fin de session, la surface se cache).
    func show(_ renderer: VideoRenderer?) {
        // L'ancien rendu (flux précédent, encore en vie quelques instants) retrouve une couche à lui.
        self.renderer?.attach(to: nil)
        self.renderer = renderer
        isHidden = renderer == nil
        guard let renderer else { return }
        renderer.attach(to: metal)
        renderer.hdrDisplay = screenHDR
        updateDrawableSize()
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        updateDrawableSize()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        panelRatio = ScreenPixels.panelRatio(window?.screen)
        updateDrawableSize()
        screenObservers.forEach(NotificationCenter.default.removeObserver)
        screenObservers = []
        if let window {
            screenObservers.append(NotificationCenter.default.addObserver(forName: NSWindow.didChangeScreenNotification, object: window, queue: .main) { [weak self] _ in
                self?.updateScreenHDR()
                self?.updatePanelRatio()
            })
            // Écrans changés (dont le mode « plus d'espace » de l'écran de la fenêtre).
            screenObservers += HDRDisplay.observe { [weak self] in
                self?.updateScreenHDR()
                self?.updatePanelRatio()
            }
        }
        updateScreenHDR()
    }

    /// L'écran de la fenêtre est-il HDR ? Dit au rendu, et à `onScreenHDR` s'il change.
    func updateScreenHDR() {
        let hdr = HDRDisplay.isHDR(window?.screen)
        renderer?.hdrDisplay = hdr
        followHeadroom(hdr)
        guard hdr != screenHDR else { return }
        screenHDR = hdr
        KyberLog.video.notice("écran de la fenêtre \(hdr ? "HDR" : "SDR", privacy: .public) (marge EDR \(String(format: "%.1f", HDRDisplay.headroom(self.window?.screen)), privacy: .public))")
        onScreenHDR?(hdr)
    }

    /// Couche `linear` sur un écran HDR : la marge EDR du moment, deux fois
    /// par seconde (elle suit la luminosité de l'écran).
    private func followHeadroom(_ hdr: Bool) {
        guard VideoRenderer.hdrLayer == .linear, hdr, window != nil else {
            headroomTimer?.invalidate()
            headroomTimer = nil
            return
        }
        readHeadroom()
        guard headroomTimer == nil else { return }
        headroomTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.readHeadroom() }
        }
    }

    private func readHeadroom() {
        guard let screen = window?.screen else { return }
        renderer?.edrHeadroom = Double(screen.maximumExtendedDynamicRangeColorComponentValue)
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        panelRatio = ScreenPixels.panelRatio(window?.screen)
        updateDrawableSize()
    }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// Pixels de la dalle par pixel dessiné par macOS sur l'écran de la
    /// fenêtre (`ScreenPixels.panelRatio`), relu quand l'écran ou son mode change.
    private var panelRatio: CGFloat = 1

    private func updateDrawableSize() {
        guard let renderer else { return }
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        // Mode « plus d'espace » : macOS dessine plus grand que la dalle puis
        // réduit. L'image est dessinée aux pixels de la dalle (un 4K réglé en
        // 3008 × 1692 points : 3840 × 2160, pas 6016 × 3384) ; Core Animation
        // l'étire au cadre de la vue. Moins de travail à chaque image, et pas
        // de tampon 6K en FP16 à réduire par le compositeur (avec la barre ou
        // les statistiques par-dessus, en HDR à 160 Hz, l'écran a gelé).
        let pixels = scale * panelRatio
        metal.contentsScale = pixels
        renderer.markerScale = pixels
        renderer.setDrawableSize(CGSize(width: (bounds.width * pixels).rounded(), height: (bounds.height * pixels).rounded()))
    }

    /// Relit le rapport dalle / dessin de l'écran de la fenêtre et redimensionne si besoin.
    private func updatePanelRatio() {
        let ratio = ScreenPixels.panelRatio(window?.screen)
        guard ratio != panelRatio else { return }
        panelRatio = ratio
        KyberLog.video.notice("image aux pixels de la dalle : rapport \(String(format: "%.3f", ratio), privacy: .public)")
        updateDrawableSize()
    }
}

/// Pixels réels des écrans du Mac.
public enum ScreenPixels {
    /// Pixels de la dalle (mode natif de l'écran), nil s'ils sont inconnus.
    public static func native(_ screen: NSScreen) -> (width: Int, height: Int)? {
        guard let id = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID,
              let modes = CGDisplayCopyAllDisplayModes(id, [kCGDisplayShowDuplicateLowResolutionModes: true] as CFDictionary) as? [CGDisplayMode],
              let native = modes.first(where: { $0.ioFlags & UInt32(kDisplayModeNativeFlag) != 0 })
        else { return nil }
        return (native.pixelWidth, native.pixelHeight)
    }

    /// Pixels de la dalle par pixel dessiné par macOS : 1, sauf en mode
    /// « plus d'espace » où macOS dessine plus grand puis réduit (un 4K réglé
    /// en 3008 × 1692 points : 3840 / 6016 ≈ 0,64). Jamais plus de 1.
    public static func panelRatio(_ screen: NSScreen?) -> CGFloat {
        guard let screen, let native = native(screen) else { return 1 }
        let drawn = screen.frame.width * screen.backingScaleFactor
        guard drawn >= 1 else { return 1 }
        return min(1, CGFloat(native.width) / drawn)
    }
}
