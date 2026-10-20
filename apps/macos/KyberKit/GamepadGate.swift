// Combinaison manette L1 + R1 + START + SELECT : elle ouvre la barre de
// l'app, et la manette pilote alors KyberNow au lieu de l'ordinateur.
//
// Branché comme « hook » du routeur kynput : chaque paquet de manette y
// passe avant de partir vers l'ordinateur. Le routeur appelle le hook sous
// son verrou : on ne réinjecte donc jamais de paquet depuis le rappel.
//
// Le hook ne décide pas : il signale la combinaison, et c'est le contrôleur
// de session qui prend la manette (`setCaptured`) quand la barre s'ouvre
// vraiment. Hors session (connexion, reconnexion), rien n'est pris et la
// manette reste à l'ordinateur ; avant, le hook basculait tout seul et la
// manette restait muette jusqu'à la combinaison suivante.

import Foundation
@_implementationOnly import CKyber

public enum PadKey: Sendable {
    case left, right, up, down, accept, back
}

final class GamepadGate: @unchecked Sendable {
    /// Combinaison détectée (fil principal).
    var onCombo: (() -> Void)?
    /// Touche de navigation pendant que la manette pilote l'app (fil principal).
    var onNavigate: ((PadKey) -> Void)?
    /// Relâchements à envoyer à l'ordinateur (hors du verrou du routeur).
    var release: (([(UInt32, kynput_GamepadButton)]) -> Void)?

    private let lock = NSLock()
    private var pressed: [UInt32: Set<UInt32>] = [:]
    private var captured = false
    /// Relâchements que nous injectons nous-mêmes : à laisser passer.
    private var allowed: [UInt32: Set<UInt32>] = [:]

    private static let combo: Set<UInt32> = [
        KYNPUT_GAMEPAD_BUTTON_LEFT_SHOULDER.rawValue, KYNPUT_GAMEPAD_BUTTON_RIGHT_SHOULDER.rawValue,
        KYNPUT_GAMEPAD_BUTTON_START.rawValue, KYNPUT_GAMEPAD_BUTTON_BACK.rawValue,
    ]

    /// L'app prend la manette (barre ouverte) ou la rend (barre fermée).
    func setCaptured(_ value: Bool) {
        lock.lock()
        let was = captured
        captured = value
        guard value, !was else {
            if !value { allowed = [:] }
            lock.unlock()
            return
        }
        // L'ordinateur a vu les touches tenues (les trois premières de la
        // combinaison) : on les relâche pour lui.
        var releases: [(UInt32, kynput_GamepadButton)] = []
        for (id, buttons) in pressed {
            for b in buttons {
                releases.append((id, kynput_GamepadButton(rawValue: b)))
                allowed[id, default: []].insert(b)
            }
        }
        lock.unlock()
        if !releases.isEmpty {
            DispatchQueue.global().async { [weak self] in self?.release?(releases) }
        }
    }

    /// `true` : le paquet part vers l'ordinateur.
    func filter(_ packet: OpaquePointer) -> Bool {
        var event = kynput_GamepadEventId(rawValue: 0)
        guard kynput_gamepad_event_id_from_packet(packet, &event) else { return true }
        lock.lock()
        defer { lock.unlock() }
        guard event == KYNPUT_GAMEPAD_EVENT_ID_BUTTON else { return !captured }
        var b = kynput_GamepadEventButton()
        guard kynput_gamepad_button_from_packet(packet, &b) else { return !captured }
        let button = b.type_.rawValue
        if !b.pressed, allowed[b.id]?.remove(button) != nil {
            return true
        }
        if b.pressed { pressed[b.id, default: []].insert(button) } else { pressed[b.id]?.remove(button) }

        if b.pressed, Self.combo.contains(button), pressed[b.id, default: []].isSuperset(of: Self.combo) {
            // La quatrième touche n'atteint pas l'ordinateur. La suite est au
            // contrôleur de session : prendre la manette, ou la rendre.
            DispatchQueue.main.async { [weak self] in self?.onCombo?() }
            return false
        }
        guard captured else { return true }
        if b.pressed, let key = Self.key(button) {
            DispatchQueue.main.async { [weak self] in self?.onNavigate?(key) }
        }
        return false
    }

    private static func key(_ button: UInt32) -> PadKey? {
        switch button {
        case KYNPUT_GAMEPAD_BUTTON_D_PAD_LEFT.rawValue: return .left
        case KYNPUT_GAMEPAD_BUTTON_D_PAD_RIGHT.rawValue: return .right
        case KYNPUT_GAMEPAD_BUTTON_D_PAD_UP.rawValue: return .up
        case KYNPUT_GAMEPAD_BUTTON_D_PAD_DOWN.rawValue: return .down
        case KYNPUT_GAMEPAD_BUTTON_A.rawValue: return .accept
        case KYNPUT_GAMEPAD_BUTTON_B.rawValue: return .back
        default: return nil
        }
    }
}
