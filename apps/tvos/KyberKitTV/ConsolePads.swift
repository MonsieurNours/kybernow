// Manettes de l'Apple TV (et de l'iPhone, de l'iPad) pendant une partie sur
// une console PlayStation (docs/playstation.md) : GameController les lit,
// `GamepadBridge` les donne au lecteur de manettes du cœur (au lieu de la
// chaîne kynput d'une session d'ordinateur), et le cœur renvoie ce que la
// console demande : vibrations (jouées par Core Haptics, comme pour un
// ordinateur) et gâchettes adaptatives de la DualSense (rapport d'effets de
// 47 octets, `DS5EffectsState_t` de SDL, traduit ici pour
// `GCDualSenseAdaptiveTrigger`).
//
// Pont : `kn_ps_pad_*` (apps/apple/src/playstation.rs), crochet du cœur
// `gamepads::set_console_feedback`.

@_implementationOnly import CKyber
@_implementationOnly import CKyberNow
import Foundation
import GameController

enum ConsolePads {
    static func plug(_ id: UInt32, vendor: UInt16?, product: UInt16?) {
        kn_ps_pad_plug(id, vendor.map(Int32.init) ?? -1, product.map(Int32.init) ?? -1)
    }

    static func unplug(_ id: UInt32) { kn_ps_pad_unplug(id) }

    static func button(_ id: UInt32, _ button: PadButton, pressed: Bool) {
        kn_ps_pad_button(id, button.core, pressed)
    }

    static func axis(_ id: UInt32, _ axis: PadAxis, _ value: Int16) {
        kn_ps_pad_axis(id, axis.core, value)
    }

    static func touchClick(_ id: UInt32, pressed: Bool) { kn_ps_pad_touch_click(id, pressed) }

    /// Doigt sur le pavé tactile, GameController : de -1 à 1, y vers le haut.
    static func touch(_ id: UInt32, finger: UInt32, x: Float, y: Float) {
        // (0, 0) : doigt levé (GameController ne dit pas mieux ; SDL fait de même).
        let down = x != 0 || y != 0
        kn_ps_pad_touch(id, finger, down, (x + 1) / 2, (1 - y) / 2)
    }

    /// Mouvements d'une manette (GCMotion) dans le repère de SDL : gyroscope
    /// en rad/s, accéléromètre en m/s² (GameController le donne en g).
    static func motion(_ id: UInt32, _ m: GCMotion) {
        let us = UInt64(ProcessInfo.processInfo.systemUptime * 1_000_000)
        if m.hasRotationRate {
            let r = m.rotationRate
            kn_ps_pad_sensor(id, true, Float(r.x), Float(r.z), Float(-r.y), us)
        }
        let g: Float = 9.80665
        let a = m.hasGravityAndUserAcceleration
            ? GCAcceleration(x: m.gravity.x + m.userAcceleration.x, y: m.gravity.y + m.userAcceleration.y, z: m.gravity.z + m.userAcceleration.z)
            : m.acceleration
        kn_ps_pad_sensor(id, false, Float(-a.x) * g, Float(-a.z) * g, Float(a.y) * g, us)
    }

    // MARK: Retour de la console

    /// Ce que la console renvoie à une manette.
    enum Feedback {
        case rumble(GamepadRumble)
        /// Rapport d'effets de la DualSense (gâchettes adaptatives).
        case effect(id: UInt32, report: [UInt8])
    }

    /// Rappel des retours (fils de la partie) ; nil : plus rien.
    static func setFeedback(_ f: ((Feedback) -> Void)?) {
        lock.lock()
        handler = f
        lock.unlock()
        if f == nil {
            kn_ps_pad_set_feedback(nil, nil)
            return
        }
        kn_ps_pad_set_feedback({ _, pad, large, small, ms, effect, len in
            let event: Feedback
            if let effect, len > 0 {
                event = .effect(id: pad, report: Array(UnsafeBufferPointer(start: effect, count: Int(len))))
            } else {
                event = .rumble(GamepadRumble(id: pad, largeMotor: large, smallMotor: small, durationMs: Int(ms)))
            }
            ConsolePads.lock.lock()
            let h = ConsolePads.handler
            ConsolePads.lock.unlock()
            h?(event)
        }, nil)
    }

    private static let lock = NSLock()
    nonisolated(unsafe) private static var handler: ((Feedback) -> Void)?

    // MARK: Gâchettes adaptatives

    /// Pose les deux gâchettes d'après le rapport d'effets (`trigger_report`
    /// du cœur : type et 10 octets de la droite en 10…20, de la gauche en
    /// 21…31). Types de la DualSense (documentation de Nielk1, reprise par
    /// SDL et chiaki-ng) : 0x05 libre, 0x01 résistance simple, 0x02 arme
    /// simple, 0x06 vibration simple, 0x21 résistance par zones, 0x25 arme,
    /// 0x26 vibration par zones ; un autre type (arc, galop, machine) rend la
    /// gâchette libre plutôt que de la bloquer.
    static func apply(report r: [UInt8], to pad: GCDualSenseGamepad) {
        guard r.count >= 32 else { return }
        set(pad.rightTrigger, type: r[10], Array(r[11..<21]))
        set(pad.leftTrigger, type: r[21], Array(r[22..<32]))
    }

    private static func set(_ t: GCDualSenseAdaptiveTrigger, type: UInt8, _ p: [UInt8]) {
        func zones() -> (active: UInt16, packed: UInt32) {
            (UInt16(p[0]) | UInt16(p[1]) << 8, UInt32(p[2]) | UInt32(p[3]) << 8 | UInt32(p[4]) << 16 | UInt32(p[5]) << 24)
        }
        switch type {
        case 0x01:
            // Position (0…255), force (0…255).
            t.setModeFeedbackWithStartPosition(Float(p[0]) / 255, resistiveStrength: Float(p[1]) / 255)
        case 0x02:
            // Début, fin, force.
            t.setModeWeaponWithStartPosition(Float(p[0]) / 255, endPosition: Float(p[1]) / 255, resistiveStrength: Float(p[2]) / 255)
        case 0x06:
            // Fréquence, amplitude, position.
            t.setModeVibrationWithStartPosition(Float(p[2]) / 255, amplitude: Float(p[1]) / 255, frequency: Float(p[0]) / 255)
        case 0x21:
            // Zones actives (bit par zone), force par zone (3 bits, force - 1).
            let (active, packed) = zones()
            var s = GCDualSenseAdaptiveTrigger.PositionalResistiveStrengths()
            withUnsafeMutableBytes(of: &s.values) { raw in
                let v = raw.bindMemory(to: Float.self)
                for i in 0..<10 where active & (1 << i) != 0 { v[i] = Float((packed >> (3 * UInt32(i))) & 7 + 1) / 8 }
            }
            t.setModeFeedback(resistiveStrengths: s)
        case 0x25:
            // Zone de début et zone de fin (deux bits), force - 1 (3 bits).
            let active = UInt16(p[0]) | UInt16(p[1]) << 8
            let start = (0..<10).first { active & (1 << $0) != 0 } ?? 2
            let end = (0..<10).last { active & (1 << $0) != 0 } ?? 8
            t.setModeWeaponWithStartPosition(Float(start) / 9, endPosition: Float(end) / 9, resistiveStrength: Float(p[2] & 7 + 1) / 8)
        case 0x26:
            // Zones actives, amplitude par zone (3 bits), fréquence en Hz (octet 9).
            let (active, packed) = zones()
            var a = GCDualSenseAdaptiveTrigger.PositionalAmplitudes()
            withUnsafeMutableBytes(of: &a.values) { raw in
                let v = raw.bindMemory(to: Float.self)
                for i in 0..<10 where active & (1 << i) != 0 { v[i] = Float((packed >> (3 * UInt32(i))) & 7 + 1) / 8 }
            }
            t.setModeVibration(amplitudes: a, frequency: Float(p[8]) / 255)
        default:
            t.setModeOff()
        }
    }

    /// Fin de la partie : gâchettes libres.
    static func release(_ pad: GCDualSenseGamepad) {
        pad.leftTrigger.setModeOff()
        pad.rightTrigger.setModeOff()
    }
}
