// Clavier et souris Bluetooth branchés à l'Apple TV (GCKeyboard, GCMouse).
//
// GameController donne les touches en usages HID (page 7), que kynput
// convertit en scancodes ; la souris donne des déplacements bruts : position
// tenue par la vue de session (fil principal), ou envoyés bruts tels quels
// depuis la file quand la souris est relative (jeu en plein écran).
//
// Comme les manettes, tout arrive sur la file des entrées de GamepadBridge
// (`handlerQueue`), pas sur le fil principal ; `input` et les réglages n'y
// sont lus que là.
//
// Clavier Apple ISO (européen) : la touche sous Échap (`@#` sur un AZERTY)
// arrive comme usage 0x64 et celle près de ⇧ (`<>`) comme 0x35, à l'inverse
// de ce qu'attend l'ordinateur (quirk hid-apple ; même échange que sur Mac,
// où le système dit la disposition). tvOS ne la dit pas : réglage « Clavier
// Apple ISO », appliqué aux claviers dont le fabricant est Apple, ou inconnu
// (le clavier fusionné de GameController ne dit pas toujours le sien).

@_implementationOnly import CKyber
import GameController

final class KeyboardMouse {
    private var queue: DispatchQueue { GamepadBridge.queue }
    /// Sur la file seulement.
    private weak var input: InputPipeline?
    private var iso = false
    private var activitySent = false
    private var observers: [NSObjectProtocol] = []
    /// Une souris bouge : le pointeur doit s'afficher (fil principal).
    var onMouseActivity: (() -> Void)?
    /// Déplacement de la souris (pixels), fil principal : vrai si la vue de
    /// session l'a pris en position absolue, faux pour l'envoyer brut.
    var onMove: ((Double, Double) -> Bool)?
    /// Réglage « Clavier Apple ISO » : échange des usages 0x35 et 0x64 pour
    /// les claviers Apple.
    var appleISO = false {
        didSet { let v = appleISO; queue.async { self.iso = v } }
    }

    /// Rappelé à chaque redémarrage de l'image (profil, repli, écran) : les
    /// observateurs précédents sont retirés d'abord, sinon ils s'empilaient
    /// et chaque clavier branché était réglé autant de fois.
    func attach(_ input: InputPipeline) {
        observers.forEach(NotificationCenter.default.removeObserver)
        queue.sync {
            self.input = input
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
        }
    }

    private func setUp(_ keyboard: GCKeyboard) {
        keyboard.handlerQueue = queue
        let apple = keyboard.vendorName.map { $0.localizedCaseInsensitiveContains("Apple") } ?? true
        keyboard.keyboardInput?.keyChangedHandler = { [weak self] _, _, keyCode, pressed in
            guard let self, let input = self.input else { return }
            var usage = UInt16(truncatingIfNeeded: keyCode.rawValue)
            if apple, self.iso, usage == 0x35 || usage == 0x64 {
                usage = usage == 0x35 ? 0x64 : 0x35
            }
            var scancode: UInt16 = 0
            guard Self.scancode(usage, &scancode) else { return }
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
            // de suite. Sinon, la vue de session tient la position.
            if input.isRelative {
                input.mouseMove(dx: Int16(clamping: Int(dx.rounded())), dy: Int16(clamping: Int((-dy).rounded())))
                return
            }
            DispatchQueue.main.async { [weak input] in
                guard let input, self.onMove?(Double(dx), Double(-dy)) != true else { return }
                input.mouseMove(dx: Int16(clamping: Int(dx.rounded())), dy: Int16(clamping: Int((-dy).rounded())))
            }
        }
        m.leftButton.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(.left, pressed: pressed) }
        m.rightButton?.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(.right, pressed: pressed) }
        m.middleButton?.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(.middle, pressed: pressed) }
        // Boutons 4 et 5 (côté du pouce) : précédent et suivant, X1 et X2 sous Windows.
        for (button, kind) in zip(m.auxiliaryButtons ?? [], [InputPipeline.MouseButton.side, .extra]) {
            button.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(kind, pressed: pressed) }
        }
        m.scroll.valueChangedHandler = { [weak self] _, x, y in
            guard let self, let input = self.input else { return }
            self.scrolled(first: x, second: y, input: input)
        }
    }

    /// Molette de GameController : premier axe vertical (positif = vers le
    /// bas), second axe horizontal (positif = vers la droite), en crans (1,0 =
    /// un cran de molette, fractions pour un pavé ou une Magic Mouse), comme
    /// le lit SDL sur iOS et tvOS (`SDL_uikitevents.m`). kynput attend la même
    /// unité et les mêmes signes (`MouseWheel` : dx positif = droite, dy
    /// positif = bas), mais au plus un cran par paquet : un grand coup de
    /// molette part en plusieurs paquets. Avant, x et y partaient tels quels,
    /// donc échangés : la molette défilait de côté.
    private func scrolled(first: Float, second: Float, input: InputPipeline) {
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
        while dx != 0 || dy != 0 {
            let sx = max(-1, min(1, dx)), sy = max(-1, min(1, dy))
            input.wheel(dx: sx, dy: sy)
            dx -= sx
            dy -= sy
        }
    }

    private var scrollLogged = 0

    private static func scancode(_ usage: UInt16, _ out: inout UInt16) -> Bool {
        Keymap.scancode(usage: usage).map { out = $0; return true } ?? false
    }
}

/// Usages HID → scancodes (le reste de Keymap est propre au Mac).
enum Keymap {
    static func scancode(usage: UInt16) -> UInt16? {
        var scancode: UInt16 = 0
        return kynput_keycode_usb_to_kyber(usage, &scancode) ? scancode : nil
    }
}
