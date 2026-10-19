// Souris relative, pour les jeux : quand l'ordinateur masque son pointeur
// (un jeu en vue subjective), le pointeur du Mac est figé et caché, et seuls
// les déplacements partent vers l'ordinateur. Dès que l'ordinateur remontre
// son pointeur, celui du Mac réapparaît à la même place.
//
// Déplacements bruts de la souris (GCMouse) : sans l'accélération de macOS,
// comme les lisent les jeux sous Windows. Sans GCMouse (certains trackpads),
// les déplacements des événements AppKit prennent le relais.

import AppKit
import GameController

final class RelativeMouse {
    /// Déplacement à envoyer (fil dédié), en comptes de la souris, y vers le bas.
    var onMove: ((Int16, Int16) -> Void)?

    private(set) var active = false
    private let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.mouse", qos: .userInteractive)
    private var remainder = (x: Float(0), y: Float(0))
    private var lastRaw = Date.distantPast
    private var observer: NSObjectProtocol?

    func start() {
        guard !active else { return }
        active = true
        CGAssociateMouseAndMouseCursorPosition(0)
        NSCursor.hide()
        remainder = (0, 0)
        GCMouse.mice().forEach(attach)
        observer = NotificationCenter.default.addObserver(forName: .GCMouseDidConnect, object: nil, queue: .main) { [weak self] note in
            if let mouse = note.object as? GCMouse { self?.attach(mouse) }
        }
    }

    func stop() {
        guard active else { return }
        active = false
        GCMouse.mice().forEach { $0.mouseInput?.mouseMovedHandler = nil }
        if let observer { NotificationCenter.default.removeObserver(observer) }
        observer = nil
        CGAssociateMouseAndMouseCursorPosition(1)
        NSCursor.unhide()
    }

    /// Déplacement vu par AppKit : utilisé seulement sans GCMouse.
    func appKitMoved(dx: CGFloat, dy: CGFloat) {
        guard active else { return }
        queue.async { [self] in
            guard Date().timeIntervalSince(lastRaw) > 0.25 else { return }
            send(Float(dx), Float(dy))
        }
    }

    private func attach(_ mouse: GCMouse) {
        mouse.handlerQueue = queue
        mouse.mouseInput?.mouseMovedHandler = { [weak self] _, dx, dy in
            guard let self, self.active else { return }
            self.lastRaw = Date()
            self.send(dx, -dy) // GCMouse : y vers le haut ; Windows : vers le bas
        }
    }

    private func send(_ dx: Float, _ dy: Float) {
        // Les fractions s'accumulent : aucun mouvement lent ne se perd.
        let x = remainder.x + dx, y = remainder.y + dy
        let ix = x.rounded(.towardZero), iy = y.rounded(.towardZero)
        remainder = (x - ix, y - iy)
        guard ix != 0 || iy != 0 else { return }
        onMove?(Int16(clamping: Int(ix)), Int16(clamping: Int(iy)))
    }
}
