// Manettes du Mac, lues par kynput via SDL : combinaison L1 + R1 + START +
// SELECT (PadCombo, partagé avec l'Apple TV) et prise de la manette par
// l'app (barre ouverte).
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

final class GamepadGate: @unchecked Sendable {
    /// Combinaison détectée (fil principal).
    var onCombo: (() -> Void)?
    /// Touche de navigation pendant que la manette pilote l'app (fil principal).
    var onNavigate: ((PadKey) -> Void)?
    /// Relâchements et axes au repos à envoyer à l'ordinateur (hors du verrou du routeur).
    var release: ((_ buttons: [(UInt32, kynput_GamepadButton)], _ axes: [(UInt32, kynput_GamepadAxis)]) -> Void)?

    private let lock = NSLock()
    /// Combinaison et touches tenues de chaque manette vue passer. Sous le verrou.
    /// Formée à la quatrième touche, qui n'atteint pas l'ordinateur.
    private let combo = PadCombo(hold: .immediate)
    private var captured = false
    /// Relâchements et axes au repos que nous injectons nous-mêmes : à
    /// laisser passer, une fois chacun, pendant que l'app a la manette.
    private var allowed: [UInt32: Set<UInt32>] = [:]
    private var allowedAxes: [UInt32: Set<UInt32>] = [:]
    /// Dernière valeur partie de chaque axe : un axe qui n'a pas bougé (au
    /// 1/512 de la course) ne repart pas. Une manette Bluetooth bavarde
    /// remplirait sinon la file d'envoi de kynput (64 paquets, jetés sans
    /// retour), et avec elle les relâchements. Les paquets SDL ne passent pas
    /// par InputPipeline : seul le hook peut en réduire le débit.
    private var lastAxis: [UInt32: [UInt32: Int16]] = [:]

    init() {
        combo.onFormed = { [weak self] _ in
            // La suite est au contrôleur de session : prendre la manette, ou la rendre.
            DispatchQueue.main.async { self?.onCombo?() }
        }
    }

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
        // les remet tous à zéro. L'envoi (et son autorisation) revient à
        // InputPipeline, hors du verrou.
        var buttons: [(UInt32, kynput_GamepadButton)] = []
        var axes: [(UInt32, kynput_GamepadAxis)] = []
        for id in combo.pads {
            for b in combo.pressed(id) { buttons.append((id, b.sdk)) }
            for a in PadCombo.axes { axes.append((id, a.sdk)) }
        }
        lock.unlock()
        if !buttons.isEmpty || !axes.isEmpty {
            DispatchQueue.global().async { [weak self] in self?.release?(buttons, axes) }
        }
    }

    /// Autorise le prochain passage d'un relâchement, ou d'un axe au repos,
    /// que nous injectons : à rappeler avant chaque envoi, réessai compris.
    /// Le hook consomme l'autorisation avant que la file d'envoi accepte ou
    /// refuse le paquet, et un paquet qu'il écarte compte comme parti (le
    /// routeur rend 0) : sans réarmement, un réessai serait avalé.
    func allow(id: UInt32, button: kynput_GamepadButton) {
        lock.lock()
        allowed[id, default: []].insert(button.rawValue)
        lock.unlock()
    }

    func allow(id: UInt32, axis: kynput_GamepadAxis) {
        lock.lock()
        allowedAxes[id, default: []].insert(axis.rawValue)
        lock.unlock()
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
                combo.note(plug.id)
            } else if control == KYNPUT_GAMEPAD_CONTROL_ID_UNPLUG, kynput_gamepad_unplug_from_packet(packet, &unplug) {
                combo.forget(unplug.id)
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
            combo.note(a.id)
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
        guard kynput_gamepad_button_from_packet(packet, &b), let button = PadButton(sdk: b.type_) else { return !captured }
        if !b.pressed, allowed[b.id]?.remove(b.type_.rawValue) != nil {
            return true
        }
        // La quatrième touche de la combinaison n'atteint pas l'ordinateur.
        if combo.button(b.id, button, pressed: b.pressed) { return false }
        guard captured else { return true }
        if b.pressed, let key = PadCombo.navigationKey(button) {
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
}
