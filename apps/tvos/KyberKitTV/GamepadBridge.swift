// Manettes et télécommande de l'Apple TV (GameController) → kynput.
//
// tvOS ne passe pas par SDL : chaque manette (Xbox, DualSense, DualShock,
// MFi) devient un joueur sur l'ordinateur, dans l'ordre où elle s'est
// allumée (J1 à J4), avec ses vibrations. La Siri Remote pilote l'app ; en
// session, son bouton Retour ouvre le menu, et son pavé peut servir de souris.
//
// L1 + R1 + Start + Select (comme sur Mac) ouvre aussi le menu : la manette
// pilote alors KyberNow et le jeu ne reçoit plus rien. Le pont ne décide pas :
// il signale la combinaison, et c'est le contrôleur de session qui prend la
// manette (`setCaptured`) quand le menu s'ouvre vraiment. Hors session
// (connexion, reconnexion), rien n'est pris et la manette reste au jeu.

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
    /// La manette sert de souris et de clavier (menu de session, profil Bureau) :
    /// stick gauche = pointeur, droit = molette, A / ✕ clic, X / □ clic droit,
    /// B / ○ clic du milieu, croix = flèches, L1 / R1 = Alt + Tab, Start =
    /// touche Windows, Select = Échap ; LT ralentit, RT accélère. Le jeu ne
    /// reçoit plus la manette ; coupé, elle redevient une manette.
    public var padMouse = false {
        didSet { if padMouse != oldValue { padMouseChanged() } }
    }
    /// Vitesse du pointeur à la manette (1 : normale).
    public var padMouseSpeed: Double = 1
    /// Le pointeur se déplace (pixels de l'écran de l'ordinateur) : la vue de
    /// session tient sa position et l'envoie en absolu.
    var pointerMove: ((Double, Double) -> Void)?

    private final class Slot {
        let controller: GCController
        let id: UInt32
        var player: Int
        var pressed = Set<PadButton>()
        var live = Live()
        var motors: Motors?
        /// La combinaison du menu a été formée : on n'agit pas au relâchement de ses touches.
        var comboSeen = false
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
        // `captured` reste au contrôleur de session : l'image redémarre
        // (changement de profil) sans que le menu se ferme.
        input.onRumble = { [weak self] in self?.rumble($0) }
        // Les manettes s'annoncent une fois les entrées reliées à l'ordinateur :
        // avant, l'annonce se perd, et l'ordinateur rejette ensuite les boutons
        // d'une manette qu'il ne connaît pas. (Une annonce en double est refusée
        // sans dommage ; après une reconnexion, elle est nécessaire.)
        input.onConnected = { [weak self, weak input] in
            guard let self, let input, self.input === input else { return }
            self.plugAll(input)
        }
        plugAll(input)
        updateMouseTimer()
    }

    private func plugAll(_ input: InputPipeline) {
        for slot in orderedSlots {
            input.gamepadPlug(id: slot.id, vendor: Self.usbIDs(slot.controller)?.vendor, product: Self.usbIDs(slot.controller)?.product)
        }
    }

    func detach() {
        for slot in slots.values { slot.motors?.stop() }
        if let input { releaseMouse(input) }
        stopMouseTimer()
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
            releaseMouse(input)
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
            slot.comboSeen = true
            // La quatrième touche n'atteint pas l'ordinateur. La suite est au
            // contrôleur de session : prendre la manette (il relâche alors
            // pour l'ordinateur les touches qu'il a vues), ou la rendre.
            onMenuRequest?()
            return
        }
        defer { if slot.pressed.isDisjoint(with: Self.combo) { slot.comboSeen = false } }
        guard !captured else { return }
        if padMouse {
            mouseButton(slot, button, pressed: pressed, input: input)
            return
        }
        input.gamepadButton(id: slot.id, button, pressed: pressed)
    }

    private func axis(_ slot: Slot, _ axis: PadAxis, _ value: Int16) {
        liveDirty = true
        guard let input, !captured, !padMouse else { return }
        input.gamepadAxis(id: slot.id, axis, value: value)
    }

    // MARK: - Manette en souris

    private var heldMouse = Set<InputPipeline.MouseButton>()
    private var heldKeys = Set<UInt16>()
    private var altHeld = false
    private var altRelease: DispatchWorkItem?
    private var mouseTimer: DispatchSourceTimer?
    private var moveRest = (x: 0.0, y: 0.0)
    private var wheelRest = (x: 0.0, y: 0.0)

    // Usages HID (page 7) des touches envoyées.
    private static let keyUp: UInt16 = 0x52, keyDown: UInt16 = 0x51, keyLeft: UInt16 = 0x50, keyRight: UInt16 = 0x4F
    private static let keyTab: UInt16 = 0x2B, keyEscape: UInt16 = 0x29
    private static let keyLeftShift: UInt16 = 0xE1, keyLeftAlt: UInt16 = 0xE2, keyLeftGUI: UInt16 = 0xE3

    private func padMouseChanged() {
        if let input {
            if padMouse {
                // Le jeu lâche la manette : rien ne reste enfoncé chez lui.
                for slot in slots.values { releaseAll(slot, input: input) }
            } else {
                releaseMouse(input)
            }
        }
        updateMouseTimer()
    }

    private func key(_ usage: UInt16, _ pressed: Bool, input: InputPipeline) {
        guard let scancode = Keymap.scancode(usage: usage) else { return }
        if pressed { heldKeys.insert(usage) } else { heldKeys.remove(usage) }
        input.key(scancode: scancode, pressed: pressed)
    }

    private func tap(_ usage: UInt16, input: InputPipeline) {
        key(usage, true, input: input)
        key(usage, false, input: input)
    }

    private func mouseButton(_ slot: Slot, _ button: PadButton, pressed: Bool, input: InputPipeline) {
        func click(_ b: InputPipeline.MouseButton) {
            if pressed { heldMouse.insert(b) } else { heldMouse.remove(b) }
            input.mouseButton(b, pressed: pressed)
        }
        // Touches de la combinaison du menu : elles agissent au relâchement,
        // sauf si la combinaison a été formée entre-temps.
        let released = !pressed && !slot.comboSeen
        switch button {
        case .a: click(.left)
        case .x: click(.right)
        case .b: click(.middle)
        case .dpadUp: key(Self.keyUp, pressed, input: input)
        case .dpadDown: key(Self.keyDown, pressed, input: input)
        case .dpadLeft: key(Self.keyLeft, pressed, input: input)
        case .dpadRight: key(Self.keyRight, pressed, input: input)
        case .start: if released { tap(Self.keyLeftGUI, input: input) }
        case .back: if released { tap(Self.keyEscape, input: input) }
        case .rightShoulder: if released { altTab(forward: true, input: input) }
        case .leftShoulder: if released { altTab(forward: false, input: input) }
        default: break
        }
    }

    /// Alt reste tenu tant qu'on enchaîne (sélecteur de fenêtres ouvert),
    /// puis il est relâché : la fenêtre choisie passe devant.
    private func altTab(forward: Bool, input: InputPipeline) {
        if !altHeld {
            key(Self.keyLeftAlt, true, input: input)
            altHeld = true
        }
        if !forward { key(Self.keyLeftShift, true, input: input) }
        tap(Self.keyTab, input: input)
        if !forward { key(Self.keyLeftShift, false, input: input) }
        altRelease?.cancel()
        let release = DispatchWorkItem { [weak self] in
            guard let self, self.altHeld, let input = self.input else { return }
            self.key(Self.keyLeftAlt, false, input: input)
            self.altHeld = false
        }
        altRelease = release
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.9, execute: release)
    }

    /// Position absolue par la vue de session ; à défaut, déplacement relatif
    /// (sous-pixels gardés d'un appel à l'autre).
    private func move(dx: Double, dy: Double, input: InputPipeline) {
        if let pointerMove {
            pointerMove(dx, dy)
            return
        }
        moveRest.x += dx
        moveRest.y += dy
        let ix = Int16(clamping: Int(moveRest.x.rounded(.towardZero)))
        let iy = Int16(clamping: Int(moveRest.y.rounded(.towardZero)))
        moveRest.x -= Double(ix)
        moveRest.y -= Double(iy)
        if ix != 0 || iy != 0 { input.mouseMove(dx: ix, dy: iy) }
    }

    private func releaseMouse(_ input: InputPipeline) {
        for b in heldMouse { input.mouseButton(b, pressed: false) }
        heldMouse.removeAll()
        altRelease?.cancel()
        altHeld = false
        for usage in heldKeys {
            if let scancode = Keymap.scancode(usage: usage) { input.key(scancode: scancode, pressed: false) }
        }
        heldKeys.removeAll()
    }

    private func updateMouseTimer() {
        guard padMouse, input != nil else {
            stopMouseTimer()
            return
        }
        guard mouseTimer == nil else { return }
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now(), repeating: 1.0 / 120, leeway: .milliseconds(1))
        timer.setEventHandler { [weak self] in self?.mouseTick() }
        timer.resume()
        mouseTimer = timer
    }

    private func stopMouseTimer() {
        mouseTimer?.cancel()
        mouseTimer = nil
        moveRest = (0, 0)
        wheelRest = (0, 0)
    }

    /// 120 fois par seconde : le stick gauche déplace le pointeur (lent près
    /// du centre, rapide à fond), le droit fait tourner la molette.
    private func mouseTick() {
        guard padMouse, !captured, let input else { return }
        let dt = 1.0 / 120
        var dx = 0.0, dy = 0.0, wx = 0.0, wy = 0.0
        for slot in slots.values {
            let left = slot.live.leftStick
            let m = Double(hypot(left.x, left.y))
            if m > 0.12 {
                let n = min(1, (m - 0.12) / 0.88)
                // Jusqu'à ~2 600 px/s à fond : l'écran 4K traversé en 1,5 s.
                let precision = 1 - 0.7 * Double(slot.live.leftTrigger)
                let boost = 1 + 1.5 * Double(slot.live.rightTrigger)
                let speed = 2600 * pow(n, 2.2) * padMouseSpeed * precision * boost
                dx += Double(left.x) / m * speed * dt
                dy -= Double(left.y) / m * speed * dt
            }
            let right = slot.live.rightStick
            // Jusqu'à 14 crans de molette par seconde.
            if abs(right.y) > 0.15 { wy += Double(right.y) * 14 * dt }
            if abs(right.x) > 0.15 { wx += Double(right.x) * 14 * dt }
        }
        if dx != 0 || dy != 0 { move(dx: dx, dy: dy, input: input) }
        wheelRest.x += wx
        wheelRest.y += wy
        if abs(wheelRest.y) >= 0.25 || abs(wheelRest.x) >= 0.25 {
            input.wheel(dx: Float(wheelRest.x), dy: Float(wheelRest.y))
            wheelRest = (0, 0)
        }
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
                self.move(dx: Double(point.x - last.x) * 900, dy: Double(last.y - point.y) * 900, input: input)
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
