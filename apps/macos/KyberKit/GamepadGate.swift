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
    /// Relâchements et axes au repos à envoyer à l'ordinateur (hors du verrou du routeur).
    var release: ((_ buttons: [(UInt32, kynput_GamepadButton)], _ axes: [(UInt32, kynput_GamepadAxis)]) -> Void)?

    private let lock = NSLock()
    /// Manettes vues passer (branchées, ou dont un paquet est passé) : à
    /// remettre à zéro quand l'app prend la manette.
    private var known = Set<UInt32>()
    private var pressed: [UInt32: Set<UInt32>] = [:]
    private var captured = false
    /// Relâchements et axes au repos que nous injectons nous-mêmes : à
    /// laisser passer, une fois chacun, pendant que l'app a la manette.
    private var allowed: [UInt32: Set<UInt32>] = [:]
    private var allowedAxes: [UInt32: Set<UInt32>] = [:]
    /// Dernière valeur partie de chaque axe : un axe qui n'a pas bougé (au
    /// 1/512 de la course) ne repart pas. Une manette Bluetooth bavarde
    /// remplirait sinon la file d'envoi de kynput (64 paquets, jetés sans
    /// retour), et avec elle les relâchements.
    private var lastAxis: [UInt32: [UInt32: Int16]] = [:]

    private static let axes: [kynput_GamepadAxis] = [
        KYNPUT_GAMEPAD_AXIS_LEFT_X, KYNPUT_GAMEPAD_AXIS_LEFT_Y, KYNPUT_GAMEPAD_AXIS_RIGHT_X,
        KYNPUT_GAMEPAD_AXIS_RIGHT_Y, KYNPUT_GAMEPAD_AXIS_TRIGGER_LEFT, KYNPUT_GAMEPAD_AXIS_TRIGGER_RIGHT,
    ]

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
            if !value {
                allowed = [:]
                allowedAxes = [:]
            }
            lock.unlock()
            return
        }
        // Tout à zéro sur l'ordinateur : les touches tenues (les trois premières
        // de la combinaison, ou une gâchette) relâchées, les six axes au repos,
        // pour chaque manette connue. kynput ne dit pas l'état des axes : on
        // les remet tous à zéro.
        var buttons: [(UInt32, kynput_GamepadButton)] = []
        var axes: [(UInt32, kynput_GamepadAxis)] = []
        for id in known {
            for b in pressed[id] ?? [] {
                buttons.append((id, kynput_GamepadButton(rawValue: b)))
                allowed[id, default: []].insert(b)
            }
            for a in Self.axes {
                axes.append((id, a))
                allowedAxes[id, default: []].insert(a.rawValue)
            }
        }
        lock.unlock()
        if !buttons.isEmpty || !axes.isEmpty {
            DispatchQueue.global().async { [weak self] in self?.release?(buttons, axes) }
        }
    }

    /// `true` : le paquet part vers l'ordinateur.
    func filter(_ packet: OpaquePointer) -> Bool {
        var control = kynput_GamepadControlId(rawValue: 0)
        if kynput_gamepad_control_id_from_packet(packet, &control) {
            // Branchements : toujours transmis, et notés pour la remise à zéro.
            lock.lock()
            defer { lock.unlock() }
            var plug = kynput_GamepadControlIdPlug()
            var unplug = kynput_GamepadControlIdUnplug()
            if control == KYNPUT_GAMEPAD_CONTROL_ID_PLUG, kynput_gamepad_plug_from_packet(packet, &plug) {
                known.insert(plug.id)
            } else if control == KYNPUT_GAMEPAD_CONTROL_ID_UNPLUG, kynput_gamepad_unplug_from_packet(packet, &unplug) {
                known.remove(unplug.id)
                pressed[unplug.id] = nil
                allowed[unplug.id] = nil
                allowedAxes[unplug.id] = nil
                lastAxis[unplug.id] = nil
            }
            return true
        }
        var event = kynput_GamepadEventId(rawValue: 0)
        guard kynput_gamepad_event_id_from_packet(packet, &event) else { return true }
        lock.lock()
        defer { lock.unlock() }
        if event == KYNPUT_GAMEPAD_EVENT_ID_AXIS {
            var a = kynput_GamepadEventAxis()
            guard kynput_gamepad_axis_from_packet(packet, &a) else { return !captured }
            known.insert(a.id)
            let axis = a.type_.rawValue
            // Manette prise : seuls nos axes au repos passent, une fois chacun
            // (un vrai retour au repos arrivé avant le nôtre fait aussi bien).
            if captured {
                guard a.value == 0, allowedAxes[a.id]?.remove(axis) != nil else { return false }
                lastAxis[a.id, default: [:]][axis] = 0
                return true
            }
            guard Self.moved(from: lastAxis[a.id]?[axis], to: a.value) else { return false }
            lastAxis[a.id, default: [:]][axis] = a.value
            return true
        }
        guard event == KYNPUT_GAMEPAD_EVENT_ID_BUTTON else { return !captured }
        var b = kynput_GamepadEventButton()
        guard kynput_gamepad_button_from_packet(packet, &b) else { return !captured }
        known.insert(b.id)
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

    /// Changement au 1/512 de la course ; un retour exact au repos passe toujours.
    static func moved(from last: Int16?, to value: Int16) -> Bool {
        guard let last else { return true }
        if value == 0 { return last != 0 }
        return last >> 7 != value >> 7
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
