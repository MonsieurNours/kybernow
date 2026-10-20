// Manettes et télécommande de l'Apple TV (GameController) → kynput.
//
// tvOS ne passe pas par SDL : chaque manette (Xbox, DualSense, DualShock,
// MFi) devient un joueur sur l'ordinateur, dans l'ordre où elle s'est
// allumée (J1 à J4), avec ses vibrations. La Siri Remote pilote l'app ; en
// session, son bouton Retour ouvre le menu, et son pavé peut servir de souris.
//
// L1 + R1 + Start + Select (comme sur Mac) ouvre aussi le menu : la manette
// pilote alors KyberNow et le jeu ne reçoit plus rien.

import Combine
import CoreHaptics
import GameController
import UIKit

public final class GamepadBridge: ObservableObject {
    public static let shared = GamepadBridge()

    public enum Kind: String, Sendable {
        case xbox, playstation, other
    }

    /// Une manette vue par l'app.
    public struct Pad: Identifiable, Equatable, Sendable {
        public let id: UInt32
        public let name: String
        public let kind: Kind
        /// Joueur sur l'ordinateur, de 1 à 4.
        public let player: Int
        public var battery: Float?
        public var charging: Bool
        public var rumbles: Bool
    }

    /// État en direct d'une manette (écran de test).
    public struct Live: Equatable, Sendable {
        public var pressed: Set<String> = []
        public var leftStick = CGPoint.zero
        public var rightStick = CGPoint.zero
        public var leftTrigger: Float = 0
        public var rightTrigger: Float = 0
        public init() {}
    }

    @Published public private(set) var pads: [Pad] = []
    @Published public private(set) var remoteConnected = false
    /// Dernière manette utilisée : les glyphes de l'interface la suivent.
    @Published public private(set) var lastKind: Kind?
    /// États en direct, publiés 30 fois par seconde tant que `monitoring`.
    @Published public private(set) var live: [UInt32: Live] = [:]
    public var monitoring = false

    /// L1 + R1 + Start + Select, ou bouton Retour de la télécommande, en session.
    public var onMenuRequest: (() -> Void)?
    /// La télécommande sert de souris (pavé = déplacement, clic = clic gauche).
    public var remoteMouse = false
    /// Le bouton Retour de la télécommande ouvre le menu de session.
    public var menuFromRemote = true
    /// L1 + R1 + Start + Select ouvre le menu de session.
    public var menuFromCombo = true
    /// Vibrations renvoyées par le jeu.
    public var rumbleEnabled = true
    /// Ordre fixe : chaque manette retrouve sa place (J1…J4) d'une session à l'autre.
    public var fixedOrder = false

    private final class Slot {
        let controller: GCController
        let id: UInt32
        var player: Int
        var pressed = Set<PadButton>()
        var live = Live()
        var motors: Motors?
        init(controller: GCController, id: UInt32, player: Int) {
            self.controller = controller
            self.id = id
            self.player = player
        }
    }

    private var slots: [ObjectIdentifier: Slot] = [:]
    private var nextID: UInt32 = 1
    private weak var input: InputPipeline?
    /// La manette pilote l'app (menu ouvert) : rien ne part vers l'ordinateur.
    public private(set) var captured = false
    private var liveTimer: Timer?
    private var liveDirty = false
    private var lastTouch: CGPoint?
    private var batteryTimer: Timer?

    private init() {
        NotificationCenter.default.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] note in
            if let c = note.object as? GCController { self?.connected(c) }
        }
        NotificationCenter.default.addObserver(forName: .GCControllerDidDisconnect, object: nil, queue: .main) { [weak self] note in
            if let c = note.object as? GCController { self?.disconnected(c) }
        }
        GCController.controllers().forEach(connected)
        GCController.startWirelessControllerDiscovery {}
        batteryTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in self?.publishPads() }
        liveTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 30, repeats: true) { [weak self] _ in
            guard let self, self.monitoring, self.liveDirty else { return }
            self.liveDirty = false
            self.live = Dictionary(uniqueKeysWithValues: self.slots.values.map { ($0.id, $0.live) })
        }
    }

    // MARK: - Session

    /// Branche les manettes sur la chaîne d'entrées de la session.
    func attach(_ input: InputPipeline) {
        self.input = input
        captured = false
        input.onRumble = { [weak self] in self?.rumble($0) }
        for slot in orderedSlots {
            input.gamepadPlug(id: slot.id, vendor: Self.usbIDs(slot.controller)?.vendor, product: Self.usbIDs(slot.controller)?.product)
        }
    }

    func detach() {
        for slot in slots.values { slot.motors?.stop() }
        if let input {
            for slot in slots.values {
                releaseAll(slot, input: input)
                input.gamepadUnplug(id: slot.id)
            }
        }
        input = nil
        captured = false
    }

    /// Le menu s'ouvre (true) ou se ferme : la manette quitte ou rejoint le jeu.
    public func setCaptured(_ value: Bool) {
        guard value != captured else { return }
        captured = value
        if value, let input {
            for slot in slots.values { releaseAll(slot, input: input) }
        }
    }

    public var inSession: Bool { input != nil }

    // MARK: - Branchements

    private var orderedSlots: [Slot] { slots.values.sorted { $0.player < $1.player } }

    private func connected(_ c: GCController) {
        if c.extendedGamepad == nil {
            if c.microGamepad != nil { setUpRemote(c) }
            return
        }
        let key = ObjectIdentifier(c)
        guard slots[key] == nil else { return }
        let used = Set(slots.values.map(\.player))
        let remembered = fixedOrder ? Self.rememberedPlayer(c) : nil
        let player = remembered.flatMap { used.contains($0) ? nil : $0 } ?? (1...8).first { !used.contains($0) } ?? used.count + 1
        Self.remember(c, player: player)
        let slot = Slot(controller: c, id: nextID, player: player)
        nextID += 1
        slots[key] = slot
        if player <= 4 { c.playerIndex = GCControllerPlayerIndex(rawValue: player - 1) ?? .indexUnset }
        c.light?.color = Self.lightColor(player)
        slot.motors = Motors(controller: c)
        setUpPad(slot)
        if let input {
            input.gamepadPlug(id: slot.id, vendor: Self.usbIDs(c)?.vendor, product: Self.usbIDs(c)?.product)
        }
        KyberLog.app.notice("manette branchée : \(c.vendorName ?? "?", privacy: .public) (J\(player))")
        publishPads()
    }

    private func disconnected(_ c: GCController) {
        if c.extendedGamepad == nil {
            remoteConnected = GCController.controllers().contains { $0.extendedGamepad == nil && $0.microGamepad != nil && $0 !== c }
            return
        }
        guard let slot = slots.removeValue(forKey: ObjectIdentifier(c)) else { return }
        slot.motors?.stop()
        if let input {
            releaseAll(slot, input: input)
            input.gamepadUnplug(id: slot.id)
        }
        publishPads()
    }

    private func publishPads() {
        pads = orderedSlots.map { s in
            Pad(id: s.id, name: Self.name(s.controller), kind: Self.kind(s.controller), player: s.player,
                battery: s.controller.battery.map { $0.batteryLevel }, charging: s.controller.battery?.batteryState == .charging,
                rumbles: s.motors != nil)
        }
    }

    // MARK: - Manettes

    private func setUpPad(_ slot: Slot) {
        guard let pad = slot.controller.extendedGamepad else { return }
        let buttons: [(GCControllerButtonInput?, PadButton)] = [
            (pad.buttonA, .a), (pad.buttonB, .b), (pad.buttonX, .x), (pad.buttonY, .y),
            (pad.leftShoulder, .leftShoulder), (pad.rightShoulder, .rightShoulder),
            (pad.buttonMenu, .start), (pad.buttonOptions, .back), (pad.buttonHome, .guide),
            (pad.leftThumbstickButton, .leftStick), (pad.rightThumbstickButton, .rightStick),
            (pad.dpad.up, .dpadUp), (pad.dpad.down, .dpadDown), (pad.dpad.left, .dpadLeft), (pad.dpad.right, .dpadRight),
        ]
        for (element, button) in buttons {
            // Le bouton Home reste à tvOS ; les autres gestes du système aussi
            // (capture d'écran), sauf pour la manette du jeu.
            element?.preferredSystemGestureState = button == .guide ? .enabled : .disabled
            element?.pressedChangedHandler = { [weak self, weak slot] _, _, pressed in
                guard let self, let slot else { return }
                self.button(slot, button, pressed: pressed)
            }
        }
        // Pavé tactile de la DualSense / DualShock : un bouton de plus (Select).
        if let ds = pad as? GCDualSenseGamepad {
            ds.touchpadButton.pressedChangedHandler = { [weak self, weak slot] _, _, pressed in
                guard let self, let slot else { return }
                self.button(slot, .back, pressed: pressed)
            }
        }
        pad.leftThumbstick.valueChangedHandler = { [weak self, weak slot] _, x, y in
            guard let self, let slot else { return }
            slot.live.leftStick = CGPoint(x: CGFloat(x), y: CGFloat(y))
            self.axis(slot, .leftX, Self.stick(x))
            self.axis(slot, .leftY, Self.stick(-y))
        }
        pad.rightThumbstick.valueChangedHandler = { [weak self, weak slot] _, x, y in
            guard let self, let slot else { return }
            slot.live.rightStick = CGPoint(x: CGFloat(x), y: CGFloat(y))
            self.axis(slot, .rightX, Self.stick(x))
            self.axis(slot, .rightY, Self.stick(-y))
        }
        pad.leftTrigger.valueChangedHandler = { [weak self, weak slot] _, value, _ in
            guard let self, let slot else { return }
            slot.live.leftTrigger = value
            self.axis(slot, .leftTrigger, Self.trigger(value))
        }
        pad.rightTrigger.valueChangedHandler = { [weak self, weak slot] _, value, _ in
            guard let self, let slot else { return }
            slot.live.rightTrigger = value
            self.axis(slot, .rightTrigger, Self.trigger(value))
        }
    }

    private static let combo: Set<PadButton> = [.leftShoulder, .rightShoulder, .start, .back]

    private func button(_ slot: Slot, _ button: PadButton, pressed: Bool) {
        if pressed { slot.pressed.insert(button) } else { slot.pressed.remove(button) }
        if pressed { slot.live.pressed.insert(button.name) } else { slot.live.pressed.remove(button.name) }
        liveDirty = true
        if lastKind != Self.kind(slot.controller) { lastKind = Self.kind(slot.controller) }
        guard let input else { return }
        if menuFromCombo, pressed, Self.combo.contains(button), slot.pressed.isSuperset(of: Self.combo) {
            KyberLog.app.notice("combinaison L1 + R1 + Start + Select")
            if !captured {
                // L'ordinateur a vu les premières touches : on les relâche pour lui.
                for b in slot.pressed where b != button { input.gamepadButton(id: slot.id, b, pressed: false) }
                captured = true
            }
            onMenuRequest?()
            return
        }
        guard !captured else { return }
        input.gamepadButton(id: slot.id, button, pressed: pressed)
    }

    private func axis(_ slot: Slot, _ axis: PadAxis, _ value: Int16) {
        liveDirty = true
        guard let input, !captured else { return }
        input.gamepadAxis(id: slot.id, axis, value: value)
    }

    private func releaseAll(_ slot: Slot, input: InputPipeline) {
        for b in slot.pressed { input.gamepadButton(id: slot.id, b, pressed: false) }
        for a in [PadAxis.leftX, .leftY, .rightX, .rightY, .leftTrigger, .rightTrigger] {
            input.gamepadAxis(id: slot.id, a, value: 0)
        }
    }

    /// Axe de -1…1 vers l'entier signé de SDL (vers le bas : positif).
    private static func stick(_ v: Float) -> Int16 {
        Int16(clamping: Int((max(-1, min(1, v)) * 32767).rounded()))
    }

    private static func trigger(_ v: Float) -> Int16 {
        Int16(clamping: Int((max(0, min(1, v)) * 32767).rounded()))
    }

    // MARK: - Siri Remote

    private func setUpRemote(_ c: GCController) {
        remoteConnected = true
        guard let micro = c.microGamepad else { return }
        micro.reportsAbsoluteDpadValues = true
        micro.allowsRotation = false
        micro.buttonMenu.pressedChangedHandler = { [weak self] _, _, pressed in
            guard let self, pressed, self.menuFromRemote, self.input != nil, !self.captured else { return }
            self.onMenuRequest?()
        }
        micro.dpad.valueChangedHandler = { [weak self] _, x, y in
            guard let self, self.remoteMouse, !self.captured, let input = self.input else { return }
            let point = CGPoint(x: CGFloat(x), y: CGFloat(y))
            // Doigt posé (0, 0 : levé) : déplacement depuis la dernière position.
            if x == 0, y == 0 {
                self.lastTouch = nil
                return
            }
            if let last = self.lastTouch {
                let dx = (point.x - last.x) * 900
                let dy = (last.y - point.y) * 900
                input.mouseMove(dx: Int16(clamping: Int(dx.rounded())), dy: Int16(clamping: Int(dy.rounded())))
            }
            self.lastTouch = point
        }
        micro.buttonA.pressedChangedHandler = { [weak self] _, _, pressed in
            guard let self, self.remoteMouse, !self.captured, let input = self.input else { return }
            input.mouseButton(.left, pressed: pressed)
        }
        micro.buttonX.pressedChangedHandler = { [weak self] _, _, pressed in
            guard let self, self.remoteMouse, !self.captured, let input = self.input else { return }
            input.mouseButton(.right, pressed: pressed)
        }
    }

    // MARK: - Vibrations

    private func rumble(_ r: GamepadRumble) {
        guard rumbleEnabled, let slot = slots.values.first(where: { $0.id == r.id }) else { return }
        slot.motors?.play(large: r.largeMotor, small: r.smallMotor, durationMs: r.durationMs)
    }

    /// Vibration d'essai (écran des manettes).
    public func testRumble(_ id: UInt32) {
        guard let slot = slots.values.first(where: { $0.id == id }) else { return }
        slot.motors?.play(large: 52000, small: 30000, durationMs: 450)
    }

    // MARK: - Identité

    /// Place retenue pour une manette (ordre fixe) : nom et modèle, faute
    /// d'identifiant stable dans GameController.
    private static func key(_ c: GCController) -> String { "\(c.vendorName ?? "?")|\(c.productCategory)" }

    private static func rememberedPlayer(_ c: GCController) -> Int? {
        (UserDefaults.standard.dictionary(forKey: "padPlayers") as? [String: Int])?[key(c)]
    }

    private static func remember(_ c: GCController, player: Int) {
        var map = UserDefaults.standard.dictionary(forKey: "padPlayers") as? [String: Int] ?? [:]
        map[key(c)] = player
        UserDefaults.standard.set(map, forKey: "padPlayers")
    }

    private static func kind(_ c: GCController) -> Kind {
        switch c.extendedGamepad {
        case is GCXboxGamepad: return .xbox
        case is GCDualSenseGamepad, is GCDualShockGamepad: return .playstation
        default: return .other
        }
    }

    private static func name(_ c: GCController) -> String {
        switch c.extendedGamepad {
        case is GCXboxGamepad: return "Xbox Wireless Controller"
        case is GCDualSenseGamepad: return "DualSense"
        case is GCDualShockGamepad: return "DualShock 4"
        default: return c.vendorName ?? "Manette"
        }
    }

    /// Identifiants USB que SDL donnerait sur Mac : l'ordinateur choisit la
    /// manette virtuelle d'après eux (Xbox 360, ou DualShock 4 pour les manettes Sony).
    private static func usbIDs(_ c: GCController) -> (vendor: UInt16, product: UInt16)? {
        switch c.extendedGamepad {
        case is GCXboxGamepad: return (0x045E, 0x0B13)
        case is GCDualSenseGamepad: return (0x054C, 0x0CE6)
        case is GCDualShockGamepad: return (0x054C, 0x09CC)
        default: return nil
        }
    }

    /// Couleur de la barre lumineuse (Sony) : Spectre pour J1, puis des teintes bien distinctes.
    private static func lightColor(_ player: Int) -> GCColor {
        switch player {
        case 1: return GCColor(red: 0.43, green: 0.36, blue: 1)
        case 2: return GCColor(red: 1, green: 0.55, blue: 0.2)
        case 3: return GCColor(red: 0.3, green: 0.84, blue: 0.54)
        default: return GCColor(red: 1, green: 0.42, blue: 0.62)
        }
    }
}

extension PadButton {
    /// Nom stable pour l'écran de test.
    public var name: String { String(describing: self) }
}

// MARK: - Moteurs de vibration (Core Haptics)

/// Deux moteurs quand la manette les expose (poignées gauche et droite),
/// sinon un seul ; une vibration dure ce que le jeu demande, ou jusqu'à son arrêt.
private final class Motors {
    private var large: Motor?
    private var small: Motor?

    init?(controller: GCController) {
        guard let haptics = controller.haptics else { return nil }
        let localities = haptics.supportedLocalities
        if localities.contains(.leftHandle), localities.contains(.rightHandle) {
            large = Motor(haptics.createEngine(withLocality: .leftHandle))
            small = Motor(haptics.createEngine(withLocality: .rightHandle))
        } else {
            large = Motor(haptics.createEngine(withLocality: .default))
        }
        if large == nil { return nil }
    }

    func play(large l: UInt16, small s: UInt16, durationMs: Int) {
        let duration = durationMs < 0 ? 5.0 : Double(durationMs) / 1000
        if let small {
            large?.play(intensity: Float(l) / 65535, sharpness: 0.2, duration: duration)
            small.play(intensity: Float(s) / 65535, sharpness: 0.8, duration: duration)
        } else {
            large?.play(intensity: Float(max(l, s)) / 65535, sharpness: 0.4, duration: duration)
        }
    }

    func stop() {
        large?.stop()
        small?.stop()
    }

    private final class Motor {
        let engine: CHHapticEngine
        private var player: CHHapticPatternPlayer?

        init?(_ engine: CHHapticEngine?) {
            guard let engine else { return nil }
            self.engine = engine
            engine.isAutoShutdownEnabled = true
            engine.resetHandler = { [weak engine] in try? engine?.start() }
        }

        func play(intensity: Float, sharpness: Float, duration: Double) {
            try? player?.stop(atTime: CHHapticTimeImmediate)
            player = nil
            guard intensity > 0.01 else { return }
            let event = CHHapticEvent(eventType: .hapticContinuous, parameters: [
                CHHapticEventParameter(parameterID: .hapticIntensity, value: min(1, intensity)),
                CHHapticEventParameter(parameterID: .hapticSharpness, value: sharpness),
            ], relativeTime: 0, duration: duration)
            do {
                try engine.start()
                let p = try engine.makePlayer(with: CHHapticPattern(events: [event], parameters: []))
                try p.start(atTime: CHHapticTimeImmediate)
                player = p
            } catch {
                KyberLog.app.error("vibration : \(error.localizedDescription, privacy: .public)")
            }
        }

        func stop() {
            try? player?.stop(atTime: CHHapticTimeImmediate)
            player = nil
        }
    }
}
