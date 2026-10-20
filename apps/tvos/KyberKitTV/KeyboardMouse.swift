// Clavier et souris Bluetooth branchés à l'Apple TV (GCKeyboard, GCMouse).
//
// GameController donne les touches en usages HID (page 7), que kynput
// convertit en scancodes ; la souris donne des déplacements bruts, envoyés
// en relatif (le pointeur de l'ordinateur est dessiné par l'app).
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
    private weak var input: InputPipeline?
    private var observers: [NSObjectProtocol] = []
    /// Une souris bouge : le pointeur doit s'afficher.
    var onMouseActivity: (() -> Void)?
    /// Déplacement de la souris (pixels) : vrai si la vue de session l'a pris
    /// en position absolue, faux pour l'envoyer brut (jeu en plein écran).
    var onMove: ((Double, Double) -> Bool)?
    /// Réglage « Clavier Apple ISO » : échange des usages 0x35 et 0x64 pour
    /// les claviers Apple.
    var appleISO = false

    /// Rappelé à chaque redémarrage de l'image (profil, repli, écran) : les
    /// observateurs précédents sont retirés d'abord, sinon ils s'empilaient
    /// et chaque clavier branché était réglé autant de fois.
    func attach(_ input: InputPipeline) {
        observers.forEach(NotificationCenter.default.removeObserver)
        self.input = input
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
        }
        input?.flushKeys()
        input = nil
    }

    private func setUp(_ keyboard: GCKeyboard) {
        let apple = keyboard.vendorName.map { $0.localizedCaseInsensitiveContains("Apple") } ?? true
        keyboard.keyboardInput?.keyChangedHandler = { [weak self] _, _, keyCode, pressed in
            guard let self, let input = self.input else { return }
            var usage = UInt16(truncatingIfNeeded: keyCode.rawValue)
            if apple, self.appleISO, usage == 0x35 || usage == 0x64 {
                usage = usage == 0x35 ? 0x64 : 0x35
            }
            var scancode: UInt16 = 0
            guard Self.scancode(usage, &scancode) else { return }
            input.key(scancode: scancode, pressed: pressed)
        }
    }

    private func setUp(_ mouse: GCMouse) {
        guard let m = mouse.mouseInput else { return }
        m.mouseMovedHandler = { [weak self] _, dx, dy in
            DispatchQueue.main.async {
                guard let self, let input = self.input else { return }
                self.onMouseActivity?()
                if self.onMove?(Double(dx), Double(-dy)) != true {
                    input.mouseMove(dx: Int16(clamping: Int(dx.rounded())), dy: Int16(clamping: Int((-dy).rounded())))
                }
            }
        }
        m.leftButton.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(.left, pressed: pressed) }
        m.rightButton?.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(.right, pressed: pressed) }
        m.middleButton?.pressedChangedHandler = { [weak self] _, _, pressed in self?.input?.mouseButton(.middle, pressed: pressed) }
        m.scroll.valueChangedHandler = { [weak self] _, x, y in self?.input?.wheel(dx: x, dy: y) }
    }

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
