// Clavier et souris de l'iPad pendant une partie sur console PlayStation
// (docs/playstation.md, « Clavier et souris ») : traduits en manette par le
// cœur (`apps/core/src/playstation/keymouse.rs`), comme sur Mac
// (PlayStationInput.swift). GameController donne les touches en usages HID
// (leur place : la disposition de la console suit celle du clavier, ZQSD en
// AZERTY) et la souris en déplacements bruts. Une manette branchée l'emporte :
// le cœur ignore alors clavier et souris.
//
// ⌃⌥⌘ + une lettre reste à l'app (`onShortcut`) ; barre ouverte ou app
// en arrière-plan (`suspended`) : rien ne part, tout est relâché. Clavier de
// la console ouvert (`consoleKeyboard`) : les touches vont au champ de texte
// de l'app (UIKit), pas à la manette.

import GameController

public final class PlayStationKeyMouse {
    private weak var play: PlayStationPlay?
    private var observers: [NSObjectProtocol] = []
    /// Modificateurs tenus (usages 0xE0 à 0xE7), pour ⌃⌥⌘.
    private var modifiers: Set<UInt16> = []
    /// Touches parties à la console, relâchées à la suspension.
    private var held: Set<UInt16> = []
    private var buttons: Set<UInt8> = []
    private let lock = NSLock()

    /// ⌃⌥⌘ + une lettre (« d », « s »…) : vrai si l'app l'a prise (fil principal).
    public var onShortcut: ((String) -> Bool)?

    /// Barre ouverte, arrière-plan : rien ne part à la console.
    public var suspended = false {
        didSet { if suspended { releaseAll() } }
    }

    /// Le clavier de la console est ouvert : les touches écrivent dans le champ de l'app.
    public var consoleKeyboard = false {
        didSet { if consoleKeyboard { releaseKeys() } }
    }

    public init() {}

    public func attach(_ play: PlayStationPlay) {
        detach()
        self.play = play
        if let k = GCKeyboard.coalesced { setUp(k) }
        GCMouse.mice().forEach(setUp)
        let center = NotificationCenter.default
        observers = [
            center.addObserver(forName: .GCKeyboardDidConnect, object: nil, queue: .main) { [weak self] note in
                if let k = note.object as? GCKeyboard { self?.setUp(k) }
            },
            center.addObserver(forName: .GCMouseDidConnect, object: nil, queue: .main) { [weak self] note in
                if let m = note.object as? GCMouse { self?.setUp(m) }
            },
        ]
    }

    public func detach() {
        releaseAll()
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        GCKeyboard.coalesced?.keyboardInput?.keyChangedHandler = nil
        for mouse in GCMouse.mice() {
            guard let m = mouse.mouseInput else { continue }
            m.mouseMovedHandler = nil
            m.leftButton.pressedChangedHandler = nil
            m.rightButton?.pressedChangedHandler = nil
            m.middleButton?.pressedChangedHandler = nil
            m.auxiliaryButtons?.forEach { $0.pressedChangedHandler = nil }
        }
        play = nil
    }

    private func setUp(_ keyboard: GCKeyboard) {
        keyboard.keyboardInput?.keyChangedHandler = { [weak self] _, _, keyCode, pressed in
            self?.key(UInt16(truncatingIfNeeded: keyCode.rawValue), pressed: pressed)
        }
    }

    private func setUp(_ mouse: GCMouse) {
        guard let m = mouse.mouseInput else { return }
        // Vers le haut : positif chez GameController, négatif pour le cœur (comme AppKit).
        m.mouseMovedHandler = { [weak self] _, dx, dy in
            guard let self, !self.suspended else { return }
            self.play?.mouseMove(dx: dx, dy: -dy)
        }
        let list: [(GCControllerButtonInput?, UInt8)] = [(m.leftButton, 0), (m.rightButton, 1), (m.middleButton, 2)]
            + (m.auxiliaryButtons ?? []).prefix(2).enumerated().map { ($0.element, UInt8(3 + $0.offset)) }
        for (element, index) in list {
            element?.pressedChangedHandler = { [weak self] _, _, pressed in self?.mouseButton(index, pressed: pressed) }
        }
    }

    private func key(_ usage: UInt16, pressed: Bool) {
        lock.lock()
        if (0xE0...0xE7).contains(usage) {
            if pressed { modifiers.insert(usage) } else { modifiers.remove(usage) }
        }
        let shortcut = pressed && Self.isShortcut(modifiers) && (0x04...0x1D).contains(usage)
        lock.unlock()
        if shortcut {
            let letter = String(UnicodeScalar(UInt8(0x61 + usage - 0x04)))
            DispatchQueue.main.async { [weak self] in _ = self?.onShortcut?(letter) }
            return
        }
        // Clavier de la console ouvert : le champ de l'app reçoit le texte.
        guard !suspended, !consoleKeyboard || !pressed else { return }
        lock.lock()
        if pressed { held.insert(usage) } else if held.remove(usage) == nil { lock.unlock(); return }
        lock.unlock()
        play?.key(usage: usage, pressed: pressed)
    }

    private func mouseButton(_ b: UInt8, pressed: Bool) {
        guard !suspended || !pressed else { return }
        lock.lock()
        if pressed { buttons.insert(b) } else if buttons.remove(b) == nil { lock.unlock(); return }
        lock.unlock()
        play?.mouseButton(b, pressed: pressed)
    }

    /// ⌃ ⌥ ⌘ tenus (gauche ou droite).
    private static func isShortcut(_ m: Set<UInt16>) -> Bool {
        (m.contains(0xE0) || m.contains(0xE4)) && (m.contains(0xE2) || m.contains(0xE6)) && (m.contains(0xE3) || m.contains(0xE7))
    }

    private func releaseKeys() {
        lock.lock()
        let keys = held
        held = []
        lock.unlock()
        for k in keys { play?.key(usage: k, pressed: false) }
    }

    private func releaseAll() {
        releaseKeys()
        lock.lock()
        let b = buttons
        buttons = []
        lock.unlock()
        for x in b { play?.mouseButton(x, pressed: false) }
        play?.releaseAll()
    }
}
