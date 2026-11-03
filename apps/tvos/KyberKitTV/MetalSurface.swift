// Vue qui porte l'image dessinée par Metal (VideoRenderer), version UIKit
// (Apple TV, iPhone et iPad).
//
// La couche Metal est une sous-couche créée par la vue elle-même, une fois
// dans une fenêtre, et le rendu ne s'y branche qu'à ce moment-là
// (`VideoRenderer.attach(to:)`, comme sur le Mac). Une couche Metal qui a
// présenté des images avant d'être posée dans une fenêtre reste hors écran
// pour toujours sur iOS : toutes ses images sont écartées (`presentedTime`
// à 0), jusqu'à un passage en arrière-plan (essai du 2026-10-22, iPhone 15
// Pro Max). Le rendu démarre sur le fil réseau dès la première image
// décodée : s'il dessinait dans sa propre couche, attachée plus tard sur le
// fil principal, l'écran resterait noir. Si l'écran écarte quand même tout
// pendant deux secondes, la couche est refaite (`heal`, trois fois au plus).
//
// HDR : la surface dit à son rendu si l'écran est HDR (`HDRDisplay.isHDR` :
// marge EDR sur iPhone et iPad, téléviseur HDR10 sur l'Apple TV), et le
// redit à chaque changement ; le rendu change de chaîne (docs/hdr.md).
// Apple TV : la surface porte aussi la couche vidéo du système
// (`HDRVideoLayer`), au-dessus de la couche Metal et cachée tant que
// l'image est en SDR ; en chaîne HDR, c'est elle qui est montrée (tvOS n'a
// pas d'EDR pour une couche Metal). Debug : `-hdrMetalTV YES` garde
// l'ancienne voie (PQ dans la couche Metal), pour comparer.

import QuartzCore
import UIKit

final class MetalSurface: UIView {
    private var metal: CAMetalLayer?
    #if os(tvOS)
    private let video: HDRVideoLayer? = {
        #if DEBUG
        if UserDefaults.standard.bool(forKey: "hdrMetalTV") { return nil }
        #endif
        return HDRVideoLayer()
    }()
    /// États du téléviseur suivis pour le journal (changement de mode).
    private var tvObservers: [NSObjectProtocol] = []
    #endif
    private var renderer: VideoRenderer?
    private var screenObservers: [NSObjectProtocol] = []
    #if os(iOS)
    /// Relève la marge EDR du moment (couche HDR `linear` : la courbe du shader s'y cale).
    private var headroomTimer: Timer?
    #endif
    /// L'écran est HDR (dernière valeur dite au rendu).
    private(set) var screenHDR = false
    /// L'écran gagne ou perd le HDR (fil principal).
    var onScreenHDR: ((Bool) -> Void)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        isUserInteractionEnabled = false
        #if os(tvOS)
        if let video {
            video.layer.isHidden = true
            layer.addSublayer(video.layer)
        }
        #endif
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    /// Branche le rendu (nil : rien à dessiner).
    func show(_ renderer: VideoRenderer?) {
        // L'ancien rendu (flux précédent, encore en vie quelques instants) retrouve une couche à lui.
        self.renderer?.attach(to: nil)
        #if os(tvOS)
        self.renderer?.attachVideo(nil)
        self.renderer?.onVideoLayer = nil
        if renderer == nil { showVideo(false) }
        #endif
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
        #if os(tvOS)
        renderer.onVideoLayer = { [weak self] on in self?.showVideo(on) }
        renderer.attachVideo(video)
        #endif
        renderer.hdrDisplay = screenHDR
        updateDrawableSize()
        updateScreenHDR()
    }

    /// L'écran est-il HDR ? Dit au rendu, et à `onScreenHDR` s'il change.
    func updateScreenHDR() {
        let hdr = HDRDisplay.isHDR(window?.screen)
        renderer?.hdrDisplay = hdr
        #if os(iOS)
        followHeadroom(hdr)
        #endif
        guard hdr != screenHDR else { return }
        screenHDR = hdr
        #if os(tvOS)
        KyberLog.video.notice("écran \(hdr ? "HDR" : "SDR", privacy: .public) (\(HDRDisplay.describeTV(self.window), privacy: .public))")
        #else
        KyberLog.video.notice("écran \(hdr ? "HDR" : "SDR", privacy: .public) (marge EDR \(String(format: "%.1f", HDRDisplay.headroom(self.window?.screen)), privacy: .public))")
        #endif
        onScreenHDR?(hdr)
    }

    private func makeLayer() -> CAMetalLayer {
        let layer = CAMetalLayer()
        layer.backgroundColor = CGColor(gray: 0, alpha: 1)
        layer.isOpaque = true
        layer.frame = self.layer.bounds
        #if os(tvOS)
        if let video {
            self.layer.insertSublayer(layer, below: video.layer)
        } else {
            self.layer.addSublayer(layer)
        }
        #else
        self.layer.addSublayer(layer)
        #endif
        metal = layer
        return layer
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        metal?.frame = layer.bounds
        #if os(tvOS)
        video?.layer.frame = layer.bounds
        #endif
        CATransaction.commit()
        updateDrawableSize()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        screenObservers.forEach(NotificationCenter.default.removeObserver)
        screenObservers = window == nil ? [] : HDRDisplay.observe { [weak self] in self?.updateScreenHDR() }
        #if os(tvOS)
        tvObservers.forEach(NotificationCenter.default.removeObserver)
        tvObservers = window == nil ? [] : HDRDisplay.logTV(window, "surface posée")
        #endif
        if metal == nil { attachIfPossible() } else { updateDrawableSize() }
        updateScreenHDR()
    }

    #if os(tvOS)
    /// Montre la couche vidéo (image HDR) ou la couche Metal (fil principal).
    private func showVideo(_ on: Bool) {
        guard let video, video.layer.isHidden == on else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        video.layer.isHidden = !on
        metal?.isHidden = on
        CATransaction.commit()
        KyberLog.video.notice("couche montrée : \(on ? "vidéo du système (HDR)" : "Metal", privacy: .public) ; \(HDRDisplay.describeTV(self.window), privacy: .public)")
    }
    #endif

    #if os(iOS)
    /// Couche `linear` sur un écran HDR : la marge EDR du moment, deux fois
    /// par seconde (elle suit la luminosité et la lumière ambiante).
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
        renderer?.edrHeadroom = Double((window?.screen ?? UIScreen.main).currentEDRHeadroom)
    }
    #endif

    private func updateDrawableSize() {
        guard let renderer, let metal else { return }
        let scale = window?.screen.nativeScale ?? UIScreen.main.nativeScale
        metal.contentsScale = scale
        renderer.setDrawableSize(CGSize(width: bounds.width * scale, height: bounds.height * scale))
    }
}
