// Clavier et souris Bluetooth branchés à l'Apple TV (GCKeyboard, GCMouse).
//
// GameController donne les touches en usages HID (page 7), que kynput
// convertit en scancodes ; la souris donne des déplacements bruts : position
// tenue par la couche de pointeur partagée (PointerLayer, sur la même file),
// ou envoyés bruts tels quels quand la souris est relative (jeu en plein écran).
//
// Comme les manettes, tout arrive sur la file des entrées de GamepadBridge
// (`handlerQueue`), pas sur le fil principal ; `input` et les réglages n'y
// sont lus que là.
//
// Clavier Apple ISO (européen) : même échange des usages 0x35 et 0x64 que sur
// Mac (Keymap, partagé) ; tvOS ne dit pas la disposition : réglage « Clavier
// Apple ISO », appliqué aux claviers Apple ou de fabricant inconnu.

import GameController

final class KeyboardMouse {
    private var queue: DispatchQueue { GamepadBridge.queue }
    /// Sur la file seulement.
    private weak var input: InputPipeline?
    /// Touches et modificateurs (usages HID) tenus : resynchronisation des
    /// modificateurs quand les entrées sont reliées de nouveau (ReleaseAll).
    private var iso = false
    private var activitySent = false
    private var observers: [NSObjectProtocol] = []
    /// Une souris bouge : le pointeur doit s'afficher (fil principal).
    var onMouseActivity: (() -> Void)?
    /// Pointeur tenu par l'app, déplacé en mode bureau. Sur la file seulement.
    private var pointer: PointerLayer?
    /// Réglage « Clavier Apple ISO » : échange des usages 0x35 et 0x64 pour
    /// les claviers Apple.
    var appleISO = false {
        didSet { let v = appleISO; queue.async { self.iso = v } }
    }

    /// Rappelé à chaque redémarrage de l'image (profil, repli, écran) : les
    /// observateurs précédents sont retirés d'abord, sinon ils s'empilaient
    /// et chaque clavier branché était réglé autant de fois.
    func attach(_ input: InputPipeline, pointer: PointerLayer) {
        observers.forEach(NotificationCenter.default.removeObserver)
        queue.sync {
            self.input = input
            self.pointer = pointer
            activitySent = false
        }
        if let keyboard = GCKeyboard.coalesced { setUp(keyboard) }
        GCMouse.mice().forEach(setUp)
        observers = [
            NotificationCenter.default.addObserver(forName: .GCKeyboardDidConnect, object: nil, queue: .main) { [weak self] note in
                if let k = note.object as? GCKeyboard { self?.setUp(k) }
            },
            NotificationCenter.default.addObserver(forName: .GCMouseDidConnect, object: nil, queue: .main) { [weak self] note in
                if let m = note.object as? GCMouse { self?.setUp(m) }
            },
        ]
    }

    func detach() {
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        GCKeyboard.coalesced?.keyboardInput?.keyChangedHandler = nil
        for mouse in GCMouse.mice() {
            mouse.mouseInput?.mouseMovedHandler = nil
            mouse.mouseInput?.leftButton.pressedChangedHandler = nil
            mouse.mouseInput?.rightButton?.pressedChangedHandler = nil
            mouse.mouseInput?.middleButton?.pressedChangedHandler = nil
            mouse.mouseInput?.scroll.valueChangedHandler = nil
            mouse.mouseInput?.auxiliaryButtons?.forEach { $0.pressedChangedHandler = nil }
        }
        queue.sync {
            input?.flushKeys()
            input = nil
            pointer = nil
        }
    }

    /// Service d'entrées relancé sur l'ordinateur (il ne tient plus rien) :
    /// les modificateurs encore tenus sur le clavier y repartent (les autres
    /// touches non : un jeu y verrait un nouvel appui). Fil principal.
    func inputConnected() {
        let keys = GCKeyboard.coalesced?.keyboardInput
        let modifiers: [GCKeyCode] = [.leftShift, .rightShift, .leftControl, .rightControl, .leftAlt, .rightAlt, .leftGUI, .rightGUI]
        let down = modifiers.filter { keys?.button(forKeyCode: $0)?.isPressed == true }.map { UInt16(truncatingIfNeeded: $0.rawValue) }
        queue.async {
            guard let input = self.input else { return }
            input.forgetModifiers()
            for usage in input.resyncModifiers(held: down) {
                guard let scancode = Keymap.scancode(usage: usage) else { continue }
                input.key(scancode: scancode, pressed: true)
            }
        }
    }

    private func setUp(_ keyboard: GCKeyboard) {
        keyboard.handlerQueue = queue
        let apple = Keymap.appleKeyboard(vendorName: keyboard.vendorName)
        keyboard.keyboardInput?.keyChangedHandler = { [weak self] _, _, keyCode, pressed in
            guard let self, let input = self.input else { return }
            let usage = UInt16(truncatingIfNeeded: keyCode.rawValue)
            guard let scancode = Keymap.scancode(hid: usage, iso: apple && self.iso) else { return }
            if Keymap.isModifier(usage) { input.modifier(usage, pressed: pressed) }
            input.key(scancode: scancode, pressed: pressed)
        }
    }

    private func setUp(_ mouse: GCMouse) {
        mouse.handlerQueue = queue
        guard let m = mouse.mouseInput else { return }
        m.mouseMovedHandler = { [weak self] _, dx, dy in
            guard let self, let input = self.input else { return }
            if !self.activitySent {
                self.activitySent = true
                DispatchQueue.main.async { self.onMouseActivity?() }
            }
            // Jeu en plein écran (souris relative) : déplacements bruts, tout
            // de suite. Sinon, la couche de pointeur tient la position et
            // l'envoie en absolu, sur cette même file : clics et molette qui
            // suivent restent derrière le déplacement.
            if !input.isRelative, let pointer = self.pointer {
                pointer.move(dx: Double(dx), dy: Double(-dy))
                return
            }
            input.mouseMove(dx: Int16(clamping: Int(dx.rounded())), dy: Int16(clamping: Int((-dy).rounded())))
        }
        m.leftButton.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(.left, pressed: pressed) }
        m.rightButton?.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(.right, pressed: pressed) }
        m.middleButton?.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(.middle, pressed: pressed) }
        // Boutons 4 et 5 (côté du pouce) : précédent et suivant, X1 et X2 sous Windows.
        for (button, kind) in zip(m.auxiliaryButtons ?? [], [InputPipeline.MouseButton.side, .extra]) {
            button.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(kind, pressed: pressed) }
        }
        m.scroll.valueChangedHandler = { [weak self] _, x, y in self?.scrolled(first: x, second: y) }
    }

    /// Molette de GameController : premier axe vertical (positif = vers le
    /// bas), second axe horizontal (positif = vers la droite), en crans (1,0 =
    /// un cran de molette, fractions pour un pavé ou une Magic Mouse), comme
    /// le lit SDL sur iOS et tvOS (`SDL_uikitevents.m`). kynput attend la même
    /// unité et les mêmes signes (`MouseWheel` : dx positif = droite, dy
    /// positif = bas), mais au plus un cran par paquet : un grand coup de
    /// molette part en plusieurs paquets. Avant, x et y partaient tels quels,
    /// donc échangés : la molette défilait de côté.
    private func scrolled(first: Float, second: Float) {
        if scrollLogged < 8, first != 0 || second != 0 {
            // Unités à confirmer sur une vraie souris : les premières valeurs au journal.
            scrollLogged += 1
            KyberLog.app.notice("molette Bluetooth : \(first) (vertical), \(second) (horizontal)")
        }
        var dy = first, dx = second
        guard dx.isFinite, dy.isFinite, dx != 0 || dy != 0 else { return }
        // Pas plus de 20 crans d'un coup (valeur aberrante).
        dx = max(-20, min(20, dx))
        dy = max(-20, min(20, dy))
        var steps: [(Float, Float)] = []
        while dx != 0 || dy != 0 {
            let sx = max(-1, min(1, dx)), sy = max(-1, min(1, dy))
            steps.append((sx, sy))
            dx -= sx
            dy -= sy
        }
        guard let input else { return }
        for (sx, sy) in steps { input.wheel(dx: sx, dy: sy) }
    }

    private var scrollLogged = 0
}
