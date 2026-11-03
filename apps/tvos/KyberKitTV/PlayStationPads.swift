// Les manettes d'une partie sur console PlayStation, sur l'iPhone, l'iPad
// et l'Apple TV (docs/playstation.md). Sur le Mac, le cœur les lit lui-même
// par SDL ; ici, GameController les lit et les donne au lecteur du cœur
// (`kn_ps_pad_*`, apps/apple/src/playstation.rs) le temps de la partie, et
// le cœur en fait une seule manette pour la console (boutons et axes réunis,
// pavé tactile et mouvements de la première qui en a). La console voit une
// DualSense ; une autre manette est lue comme elle (A = Croix, B = Rond,
// X = Carré, Y = Triangle, Affichage = Create, Menu = Options).
//
// En retour, pendant la partie seulement (`kn_ps_pad_set_feedback`) :
// - les vibrations, jouées par Core Haptics sur les moteurs de la manette
//   (`Motors` de GamepadBridge ; la manette à l'écran : le moteur de
//   l'iPhone) ;
// - les gâchettes adaptatives d'une DualSense : le rapport d'effets que la
//   console envoie (47 octets) traduit en modes de
//   `GCDualSenseAdaptiveTrigger` (résistance, arme, vibration, par zones).
//   Arc, galop et machine n'ont pas d'équivalent : approchés (arme,
//   vibration) ou rendus libres.
//
// Vibrations HD : iOS et tvOS ne donnent pas la carte son de la DualSense en
// USB ; le cœur les traduit en vibrations simples, comme en Bluetooth sur Mac.
//
// La manette à l'écran de l'iPhone et de l'iPad (ScreenPad) a sa place à
// part (`screenPad…`), comme dans GamepadBridge.

import CoreHaptics
import Foundation
import GameController
@_implementationOnly import CKyberNow

public final class PlayStationPads: @unchecked Sendable {
    public static let shared = PlayStationPads()

    /// Une manette lue par GameController.
    private final class Pad {
        let id: UInt32
        weak var controller: GCController?
        let dualSense: Bool
        var motors: Motors?
        /// Ce qui est parti au cœur, pour n'envoyer que les changements.
        var pressed: Set<PadButton> = []
        var axes: [PadAxis: Int16] = [:]
        var touchClick = false
        var fingers: [Bool] = [false, false]
        /// Le geste du système du bouton Home, rendu à la fin.
        var homeGesture: GCControllerElement.SystemGestureState?

        init(id: UInt32, controller: GCController) {
            self.id = id
            self.controller = controller
            dualSense = controller.extendedGamepad is GCDualSenseGamepad
        }
    }

    private let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.ps-pads", qos: .userInteractive)
    /// Manettes branchées, par place (0 à 3).
    private var pads: [UInt32: Pad] = [:]
    private var observers: [NSObjectProtocol] = []
    private var running = false
    /// La manette à l'écran (iPhone, iPad).
    private static let screenID: UInt32 = 8
    private var screenOn = false
    private var screenMotors: Motors?
    private var screenPressed: Set<PadButton> = []

    /// Manettes branchées ou débranchées (fil principal) : une manette
    /// branchée remplace le clavier et la souris.
    public var onChange: ((Int) -> Void)?

    /// Nombre de manettes lues par GameController.
    public var count: Int { queue.sync { pads.count } }

    // MARK: - Partie

    /// Début de la partie : les manettes branchées vont au cœur, et la
    /// suivantes à leur arrivée ; le retour (vibrations, gâchettes) vient ici.
    public func start() {
        queue.sync {
            guard !running else { return }
            running = true
            let me = Unmanaged.passUnretained(self).toOpaque()
            kn_ps_pad_set_feedback({ ud, pad, large, small, ms in
                guard let ud else { return }
                Unmanaged<PlayStationPads>.fromOpaque(ud).takeUnretainedValue().rumble(pad, large, small, Int(ms))
            }, { ud, pad, data, len in
                guard let ud, let data, len >= 32 else { return }
                let bytes = [UInt8](UnsafeBufferPointer(start: data, count: Int(len)))
                Unmanaged<PlayStationPads>.fromOpaque(ud).takeUnretainedValue().effect(pad, bytes)
            }, me)
        }
        DispatchQueue.main.async { [self] in
            let center = NotificationCenter.default
            observers.append(center.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] note in
                guard let c = note.object as? GCController else { return }
                self?.queue.async { self?.connected(c) }
            })
            observers.append(center.addObserver(forName: .GCControllerDidDisconnect, object: nil, queue: .main) { [weak self] note in
                guard let c = note.object as? GCController else { return }
                self?.queue.async { self?.disconnected(c) }
            })
            let list = GCController.controllers()
            queue.async { list.forEach(self.connected) }
        }
    }

    /// Fin de la partie : tout est relâché et débranché du cœur, les
    /// gâchettes rendues libres, le retour coupé.
    public func stop() {
        DispatchQueue.main.async { [self] in
            observers.forEach { NotificationCenter.default.removeObserver($0) }
            observers = []
        }
        queue.sync {
            guard running else { return }
            running = false
            kn_ps_pad_set_feedback(nil, nil, nil)
            for pad in pads.values { unplug(pad) }
            pads = [:]
            if screenOn { removeScreen() }
        }
        notify()
    }

    private func notify() {
        let n = queue.sync { pads.count }
        DispatchQueue.main.async { [weak self] in self?.onChange?(n) }
    }

    // MARK: - Manettes

    private func connected(_ c: GCController) {
        guard running, let gamepad = c.extendedGamepad, !pads.values.contains(where: { $0.controller === c }) else { return }
        guard let id = (0..<4).map(UInt32.init).first(where: { pads[$0] == nil }) else {
            KyberLog.app.notice("PlayStation : manette ignorée (déjà quatre)")
            return
        }
        let pad = Pad(id: id, controller: c)
        pads[id] = pad
        let ids: (Int32, Int32) = switch gamepad {
        case is GCDualSenseGamepad: (0x054C, 0x0CE6)
        case is GCDualShockGamepad: (0x054C, 0x09CC)
        case is GCXboxGamepad: (0x045E, 0x0B13)
        default: (-1, -1)
        }
        kn_ps_pad_plug(id, ids.0, ids.1)
        pad.motors = Motors(controller: c, label: c.vendorName ?? "manette")
        pad.motors?.setSession(true)
        // Le bouton PS va à la console, pas au système.
        pad.homeGesture = gamepad.buttonHome?.preferredSystemGestureState
        gamepad.buttonHome?.preferredSystemGestureState = .disabled
        gamepad.valueChangedHandler = { [weak self, weak pad] gamepad, _ in
            guard let self, let pad else { return }
            self.queue.async { self.read(pad, gamepad) }
        }
        if let motion = c.motion {
            if motion.sensorsRequireManualActivation { motion.sensorsActive = true }
            motion.valueChangedHandler = { [weak self, weak pad] m in
                guard let self, let pad else { return }
                self.motion(pad, m)
            }
        }
        read(pad, gamepad)
        KyberLog.app.notice("PlayStation : manette branchée (\(c.vendorName ?? "?", privacy: .public), place \(id))")
        notify()
    }

    private func disconnected(_ c: GCController) {
        guard let pad = pads.values.first(where: { $0.controller === c }) else { return }
        unplug(pad)
        pads[pad.id] = nil
        notify()
    }

    private func unplug(_ pad: Pad) {
        if let c = pad.controller, let gamepad = c.extendedGamepad {
            gamepad.valueChangedHandler = nil
            c.motion?.valueChangedHandler = nil
            if let g = pad.homeGesture { gamepad.buttonHome?.preferredSystemGestureState = g }
            if let ds = gamepad as? GCDualSenseGamepad {
                ds.leftTrigger.setModeOff()
                ds.rightTrigger.setModeOff()
            }
        }
        pad.motors?.stop()
        pad.motors?.setSession(false)
        kn_ps_pad_unplug(pad.id)
    }

    /// Les boutons de la manette, dans le vocabulaire de kynput (noms de la
    /// Xbox : A est Croix, B Rond, X Carré, Y Triangle).
    private static func buttons(_ g: GCExtendedGamepad) -> [(GCControllerButtonInput?, PadButton)] {
        [
            (g.buttonA, .a), (g.buttonB, .b), (g.buttonX, .x), (g.buttonY, .y),
            (g.leftShoulder, .leftShoulder), (g.rightShoulder, .rightShoulder),
            (g.buttonMenu, .start), (g.buttonOptions, .back), (g.buttonHome, .guide),
            (g.leftThumbstickButton, .leftStick), (g.rightThumbstickButton, .rightStick),
            (g.dpad.up, .dpadUp), (g.dpad.down, .dpadDown), (g.dpad.left, .dpadLeft), (g.dpad.right, .dpadRight),
        ]
    }

    /// L'état entier de la manette : les changements partent au cœur.
    private func read(_ pad: Pad, _ g: GCExtendedGamepad) {
        guard running, pads[pad.id] === pad else { return }
        var now: Set<PadButton> = []
        for (element, button) in Self.buttons(g) where element?.isPressed == true { now.insert(button) }
        for b in now.subtracting(pad.pressed) { kn_ps_pad_button(pad.id, b.core, true) }
        for b in pad.pressed.subtracting(now) { kn_ps_pad_button(pad.id, b.core, false) }
        pad.pressed = now
        let axes: [(PadAxis, Int16)] = [
            (.leftX, Self.stick(g.leftThumbstick.xAxis.value)), (.leftY, Self.stick(-g.leftThumbstick.yAxis.value)),
            (.rightX, Self.stick(g.rightThumbstick.xAxis.value)), (.rightY, Self.stick(-g.rightThumbstick.yAxis.value)),
            (.leftTrigger, Self.trigger(g.leftTrigger.value)), (.rightTrigger, Self.trigger(g.rightTrigger.value)),
        ]
        for (axis, v) in axes where pad.axes[axis] != v {
            pad.axes[axis] = v
            kn_ps_pad_axis(pad.id, axis.core, v)
        }
        guard let ds = g as? GCDualSenseGamepad else { return }
        let click = ds.touchpadButton.isPressed
        if click != pad.touchClick {
            pad.touchClick = click
            kn_ps_pad_touch_click(pad.id, click)
        }
        // Un doigt levé se lit (0, 0) : GameController ne dit pas mieux.
        for (i, finger) in [ds.touchpadPrimary, ds.touchpadSecondary].enumerated() {
            let x = finger.xAxis.value, y = finger.yAxis.value
            let down = x != 0 || y != 0
            guard down || pad.fingers[i] else { continue }
            pad.fingers[i] = down
            kn_ps_pad_touch(pad.id, UInt32(i), down, (x + 1) / 2, (1 - y) / 2)
        }
    }

    /// Gyroscope (rad/s) et accéléromètre (g → m/s²), sur le fil de GameController.
    private func motion(_ pad: Pad, _ m: GCMotion) {
        let us = UInt64(ProcessInfo.processInfo.systemUptime * 1_000_000)
        let r = m.rotationRate
        let a = m.hasGravityAndUserAcceleration
            ? (x: m.gravity.x + m.userAcceleration.x, y: m.gravity.y + m.userAcceleration.y, z: m.gravity.z + m.userAcceleration.z)
            : (x: m.acceleration.x, y: m.acceleration.y, z: m.acceleration.z)
        let g = 9.80665
        kn_ps_pad_sensor(pad.id, false, Float(a.x * g), Float(a.y * g), Float(a.z * g), us)
        kn_ps_pad_sensor(pad.id, true, Float(r.x), Float(r.y), Float(r.z), us)
    }

    /// Axe de -1…1 vers l'entier signé de SDL (vers le bas : positif).
    private static func stick(_ v: Float) -> Int16 {
        Int16(clamping: Int((max(-1, min(1, v)) * 32767).rounded()))
    }

    private static func trigger(_ v: Float) -> Int16 {
        Int16(clamping: Int((max(0, min(1, v)) * 32767).rounded()))
    }

    // MARK: - Manette à l'écran

    /// La manette à l'écran s'affiche (`true`) ou se retire : une manette de
    /// plus pour la console (le clavier et la souris ne comptent plus).
    public func setScreenPad(_ on: Bool) {
        queue.async { [self] in
            guard running, on != screenOn else { return }
            if on {
                screenOn = true
                kn_ps_pad_plug(Self.screenID, 0x054C, 0x0CE6)
                #if os(iOS)
                screenMotors = Motors.device()
                screenMotors?.setSession(true)
                #endif
            } else {
                removeScreen()
            }
        }
    }

    private func removeScreen() {
        screenOn = false
        screenPressed = []
        screenMotors?.stop()
        screenMotors?.setSession(false)
        screenMotors = nil
        kn_ps_pad_unplug(Self.screenID)
    }

    public func screenPadButton(_ button: PadButton, pressed: Bool) {
        queue.async { [self] in
            guard screenOn else { return }
            if pressed { screenPressed.insert(button) } else { screenPressed.remove(button) }
            kn_ps_pad_button(Self.screenID, button.core, pressed)
        }
    }

    /// Stick (-1…1, haut positif, comme GameController).
    public func screenPadStick(right: Bool, x: Float, y: Float) {
        queue.async { [self] in
            guard screenOn else { return }
            kn_ps_pad_axis(Self.screenID, (right ? PadAxis.rightX : .leftX).core, Self.stick(x))
            kn_ps_pad_axis(Self.screenID, (right ? PadAxis.rightY : .leftY).core, Self.stick(-y))
        }
    }

    public func screenPadTrigger(right: Bool, value: Float) {
        queue.async { [self] in
            guard screenOn else { return }
            kn_ps_pad_axis(Self.screenID, (right ? PadAxis.rightTrigger : .leftTrigger).core, Self.trigger(value))
        }
    }

    /// Le pavé tactile de la manette à l'écran : clic, et un doigt (0…1).
    public func screenPadTouch(click: Bool? = nil, at point: CGPoint?) {
        queue.async { [self] in
            guard screenOn else { return }
            if let click { kn_ps_pad_touch_click(Self.screenID, click) }
            kn_ps_pad_touch(Self.screenID, 0, point != nil, Float(point?.x ?? 0), Float(point?.y ?? 0))
        }
    }

    /// Tout relâcher (arrière-plan, barre ouverte par le toucher).
    public func releaseScreenPad() {
        queue.async { [self] in
            guard screenOn else { return }
            for b in screenPressed { kn_ps_pad_button(Self.screenID, b.core, false) }
            screenPressed = []
            for a in PadAxis.allCases { kn_ps_pad_axis(Self.screenID, a.core, 0) }
            kn_ps_pad_touch_click(Self.screenID, false)
            kn_ps_pad_touch(Self.screenID, 0, false, 0, 0)
        }
    }

    // MARK: - Retour de la console (fils de la partie)

    private func rumble(_ id: UInt32, _ large: UInt16, _ small: UInt16, _ ms: Int) {
        queue.async { [self] in
            guard running else { return }
            if id == Self.screenID {
                screenMotors?.play(large: large, small: small, durationMs: ms)
            } else {
                pads[id]?.motors?.play(large: large, small: small, durationMs: ms)
            }
        }
    }

    private func effect(_ id: UInt32, _ report: [UInt8]) {
        queue.async { [self] in
            guard running, let ds = pads[id]?.controller?.extendedGamepad as? GCDualSenseGamepad else { return }
            // Droite : type en 10, paramètres en 11 à 20 ; gauche : 21, 22 à 31.
            Self.apply(ds.rightTrigger, kind: report[10], Array(report[11..<21]))
            Self.apply(ds.leftTrigger, kind: report[21], Array(report[22..<32]))
        }
    }

    /// Un effet de gâchette de la console (`kind` et ses 10 octets ; voir
    /// apps/core/src/playstation/dualsense.rs) dans le vocabulaire de
    /// GameController.
    static func apply(_ t: GCDualSenseAdaptiveTrigger, kind: UInt8, _ p: [UInt8]) {
        let byte = { (i: Int) in Float(p[i]) / 255 }
        /// Zones actives (octets 0-1) et champs de 3 bits (octets 2-5, valeur + 1 : 1 à 8).
        func zones() -> [Float] {
            let active = UInt16(p[0]) | UInt16(p[1]) << 8
            let fields = UInt32(p[2]) | UInt32(p[3]) << 8 | UInt32(p[4]) << 16 | UInt32(p[5]) << 24
            return (0..<10).map { z in active & (1 << z) == 0 ? 0 : Float((fields >> (3 * UInt32(z))) & 7 + 1) / 8 }
        }
        /// Zones de début et de fin (bits des octets 0-1), de 0 à 1.
        func span() -> (Float, Float)? {
            let bits = UInt16(p[0]) | UInt16(p[1]) << 8
            guard bits != 0 else { return nil }
            return (Float(bits.trailingZeroBitCount) / 9, Float(15 - bits.leadingZeroBitCount) / 9)
        }
        switch kind {
        case 0x01: t.setModeFeedbackWithStartPosition(byte(0), resistiveStrength: byte(1))
        case 0x02: t.setModeWeaponWithStartPosition(byte(0), endPosition: byte(1), resistiveStrength: byte(2))
        case 0x06: t.setModeVibrationWithStartPosition(byte(2), amplitude: byte(1), frequency: byte(0))
        case 0x11: t.setModeFeedbackWithStartPosition(byte(0), resistiveStrength: Float(min(p[1], 10)) / 10)
        case 0x12: t.setModeWeaponWithStartPosition(byte(0), endPosition: byte(1), resistiveStrength: Float(min(p[2], 10)) / 10)
        case 0x21:
            let z = zones()
            var s = GCDualSenseAdaptiveTrigger.PositionalResistiveStrengths()
            withUnsafeMutableBytes(of: &s.values) { raw in
                for (i, v) in z.enumerated() { raw.storeBytes(of: v, toByteOffset: i * MemoryLayout<Float>.stride, as: Float.self) }
            }
            t.setModeFeedback(resistiveStrengths: s)
        case 0x25, 0x22:
            // Arc : approché par l'arme (sans le claquement).
            guard let (start, end) = span() else { t.setModeOff(); return }
            t.setModeWeaponWithStartPosition(start, endPosition: end, resistiveStrength: Float((p[2] & 7) + 1) / 8)
        case 0x26:
            let z = zones()
            var a = GCDualSenseAdaptiveTrigger.PositionalAmplitudes()
            withUnsafeMutableBytes(of: &a.values) { raw in
                for (i, v) in z.enumerated() { raw.storeBytes(of: v, toByteOffset: i * MemoryLayout<Float>.stride, as: Float.self) }
            }
            t.setModeVibration(amplitudes: a, frequency: byte(8))
        case 0x27:
            // Machine : une vibration à la plus forte de ses deux amplitudes.
            guard let (start, _) = span() else { t.setModeOff(); return }
            let amp = Float(max(p[2] & 7, (p[2] >> 3) & 7)) / 7
            t.setModeVibrationWithStartPosition(start, amplitude: amp, frequency: byte(3))
        default:
            // 0x05 (libre), galop et effets inconnus.
            t.setModeOff()
        }
    }
}
