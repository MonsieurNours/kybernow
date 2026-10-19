// Combinaison manette L1 + R1 + START + SELECT : elle ouvre la barre de
// l'app, et la manette pilote alors KyberNow au lieu de l'ordinateur.
//
// Branché comme « hook » du routeur kynput : chaque paquet de manette y
// passe avant de partir vers l'ordinateur. Le routeur appelle le hook sous
// son verrou : on ne réinjecte donc jamais de paquet depuis le rappel.

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

    /// L'app rend (ou prend) la manette, par exemple quand la barre se ferme.
    func setCaptured(_ value: Bool) {
        lock.lock()
        captured = value
        lock.unlock()
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
            captured.toggle()
            if captured {
                // L'ordinateur a vu les trois premières touches : on les relâche pour lui.
                let held = Self.combo.subtracting([button])
                allowed[b.id, default: []].formUnion(held)
                let id = b.id
                let releases = held.map { (id, kynput_GamepadButton(rawValue: $0)) }
                DispatchQueue.global().async { [weak self] in self?.release?(releases) }
            }
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
