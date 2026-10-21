// Manettes et télécommande de l'Apple TV (GameController) → kynput.
//
// tvOS ne passe pas par SDL : chaque manette (Xbox, DualSense, DualShock,
// MFi) devient un joueur sur l'ordinateur, dans l'ordre où elle s'est
// allumée (J1 à J4), avec ses vibrations. La Siri Remote pilote l'app ; en
// session, son bouton Retour ouvre le menu, et son pavé peut servir de souris.
//
// L1 + R1 + Start + Select (comme sur Mac) ouvre aussi le menu : la manette
// pilote alors KyberNow et le jeu ne reçoit plus rien. La combinaison et les
// touches tenues de chaque manette sont suivies par PadCombo (partagé avec le
// Mac), formée ici après 250 ms tenues. Le pont ne décide pas : il signale la
// combinaison, et c'est le contrôleur de session qui prend la manette
// (`setCaptured`) quand le menu s'ouvre vraiment. Hors session (connexion,
// reconnexion), rien n'est pris et la manette reste au jeu.
//
// Fils : tout GameController (manettes, télécommande, clavier et souris
// Bluetooth) arrive sur une seule file série `userInteractive`
// (`InputQueue.device`, partagée avec la couche de pointeur), jamais sur le
// fil principal, où une animation SwiftUI retardait une touche de plusieurs
// millisecondes (pics au-delà de 16 ms). L'état interne (places, menu pris,
// manette en souris, axes en attente, moteurs) n'est touché que sur cette
// file ; seuls `pads`, `live`, `lastKind`, `remoteConnected`, et le rappel
// vers l'interface (`onMenuRequest`) remontent sur le fil principal. Les
// réglages venus de l'interface sont recopiés sur la file.

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
        /// Joueur sur l'ordinateur, de 1 à 4 ; 0 : en attente (cinquième
        /// manette, pas branchée sur l'ordinateur).
        public let player: Int
        public var battery: Float?
        public var charging: Bool
        public var rumbles: Bool
        /// Cinquième manette (ou plus) : l'ordinateur n'en voit que quatre.
        public var waiting: Bool { player == 0 }
        /// « J2 », ou « en attente ».
        public var label: String { waiting ? "en attente" : "J\(player)" }
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

    /// File unique des entrées de GameController (manettes, télécommande,
    /// clavier et souris Bluetooth) et de la couche de pointeur : série,
    /// `userInteractive` (`InputQueue.device`, KyberKit).
    static let queue = InputQueue.device
    private var queue: DispatchQueue { Self.queue }

    // Publié sur le fil principal.
    @Published public private(set) var pads: [Pad] = []
    @Published public private(set) var remoteConnected = false
    /// Dernière manette utilisée : les glyphes de l'interface la suivent.
    @Published public private(set) var lastKind: Kind?
    /// États en direct, publiés 30 fois par seconde tant que `monitoring`.
    @Published public private(set) var live: [UInt32: Live] = [:]
    /// Message court pour la session (capsule) : manette en attente, ou qui
    /// prend une place libérée. Effacé au bout de 6 s.
    @Published public private(set) var notice: String?
    private var noticeClear: DispatchWorkItem?

    // Réglés du fil principal, recopiés sur la file (`config`).
    public var monitoring = false {
        didSet { let v = monitoring; queue.async { self.setMonitoring(v) } }
    }
    /// L1 + R1 + Start + Select, ou bouton Retour de la télécommande, en
    /// session. Appelé sur le fil principal.
    public var onMenuRequest: (() -> Void)?
    /// La télécommande sert de souris (pavé = déplacement, clic = clic gauche).
    public var remoteMouse = false {
        didSet { let v = remoteMouse; configure { $0.remoteMouse = v } }
    }
    /// Le bouton Retour de la télécommande ouvre le menu de session.
    public var menuFromRemote = true {
        didSet { let v = menuFromRemote; configure { $0.menuFromRemote = v } }
    }
    /// L1 + R1 + Start + Select ouvre le menu de session.
    public var menuFromCombo = true {
        didSet { let v = menuFromCombo; configure { $0.menuFromCombo = v } }
    }
    /// Vibrations renvoyées par le jeu.
    public var rumbleEnabled = true {
        didSet { let v = rumbleEnabled; configure { $0.rumbleEnabled = v } }
    }
    /// Ordre fixe : chaque manette retrouve sa place (J1…J4) d'une session à l'autre.
    public var fixedOrder = false {
        didSet { let v = fixedOrder; configure { $0.fixedOrder = v } }
    }
    /// La manette sert de souris et de clavier (menu de session, profil Bureau) :
    /// stick gauche = pointeur, droit = molette, A / ✕ clic, X / □ clic droit,
    /// B / ○ clic du milieu, croix = flèches, L1 / R1 = Alt + Tab, Start =
    /// touche Windows, Select = Échap ; LT ralentit, RT accélère. Le jeu ne
    /// reçoit plus la manette ; coupé, elle redevient une manette.
    public var padMouse = false {
        didSet { let v = padMouse; queue.async { self.setPadMouse(v) } }
    }
    /// Bouton Guide de la manette (Xbox, PS) : qui le reçoit.
    public enum GuideMode: String, CaseIterable, Sendable {
        /// Le bouton Home reste à tvOS (accueil, Centre de contrôle).
        case system
        /// Le bouton Home va au jeu (Steam Big Picture, Game Bar), si tvOS le
        /// laisse : demandé par `preferredSystemGestureState = .disabled`
        /// pendant la session (et l'écran de test), jamais garanti.
        case game
        /// Le bouton Home reste à tvOS ; Menu ou Options tenus 600 ms
        /// envoient Guide au jeu.
        case longPress
    }
    public var guideMode: GuideMode = .game {
        didSet { let v = guideMode; queue.async { self.setGuideMode(v) } }
    }
    /// Vitesse du pointeur à la manette (1 : normale).
    public var padMouseSpeed: Double = 1 {
        didSet { let v = padMouseSpeed; configure { $0.padMouseSpeed = v } }
    }
    /// Couche de pointeur de la vue de session (fil principal) : en mode
    /// bureau, elle tient la position du pointeur et l'envoie en absolu, sur
    /// cette même file ; nil : déplacements relatifs.
    func setPointer(_ layer: PointerLayer?) {
        queue.sync { pointer = layer }
    }
    /// Sur la file seulement.
    private var pointer: PointerLayer?

    /// Copie des réglages, lue sur la file seulement.
    private struct Config {
        var monitoring = false
        var remoteMouse = false
        var menuFromRemote = true
        var menuFromCombo = true
        var rumbleEnabled = true
        var fixedOrder = false
        var padMouse = false
        var padMouseSpeed = 1.0
        var guideMode = GuideMode.game
    }
    private var config = Config()

    private func configure(_ change: @escaping (inout Config) -> Void) {
        queue.async { change(&self.config) }
    }

    /// Vérifie (Debug) qu'on est bien sur la file des entrées.
    @inline(__always) private func onQueue() {
        #if DEBUG
        dispatchPrecondition(condition: .onQueue(Self.queue))
        #endif
    }

    private final class Slot {
        let controller: GCController
        let id: UInt32
        /// 1 à 4 ; 0 : en attente, rien ne part vers l'ordinateur.
        var player: Int
        var active: Bool { player > 0 }
        var live = Live()
        var motors: Motors?
        /// Bouton Guide par appui long : Menu ou Options tenus, pas encore
        /// envoyés (minuterie de 600 ms), et ceux qui tiennent le Guide.
        var deferred: [PadButton: DispatchWorkItem] = [:]
        var guideHeld = Set<PadButton>()
        /// Appui court rendu au relâchement : le relâchement suit 50 ms après.
        var tapRelease: [PadButton: DispatchWorkItem] = [:]
        init(controller: GCController, id: UInt32, player: Int) {
            self.controller = controller
            self.id = id
            self.player = player
        }
    }

    // Sur la file seulement.
    private var slots: [ObjectIdentifier: Slot] = [:]
    /// Combinaison du menu et touches tenues, par manette (`slot.id`).
    private let combo = PadCombo(hold: .after(PadCombo.holdDelay, on: GamepadBridge.queue))
    private var nextID: UInt32 = 1
    private weak var input: InputPipeline?
    /// La manette pilote l'app (menu ouvert) : rien ne part vers l'ordinateur.
    private var captured = false
    private var liveTimer: DispatchSourceTimer?
    private var liveDirty = false
    private var lastTouch: CGPoint?
    private var lastTouchTime: TimeInterval = 0
    /// Écran de l'ordinateur (pixels) : la vitesse du pointeur s'y rapporte.
    private var hostSize = CGSize(width: 1920, height: 1080)
    private var batteryTimer: DispatchSourceTimer?
    private var publishedKind: Kind?
    private var remotePresent = false

    private init() {
        // Les notifications arrivent sur le fil principal ; tout le reste se
        // passe sur la file.
        NotificationCenter.default.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] note in
            guard let self, let c = note.object as? GCController else { return }
            self.queue.async { self.connected(c) }
        }
        NotificationCenter.default.addObserver(forName: .GCControllerDidDisconnect, object: nil, queue: .main) { [weak self] note in
            guard let self, let c = note.object as? GCController else { return }
            self.queue.async { self.disconnected(c) }
        }
        combo.onFormed = { [weak self] id in self?.comboFormed(id) }
        let present = GCController.controllers()
        queue.async { present.forEach(self.connected) }
        GCController.startWirelessControllerDiscovery {}
        // Arrière-plan (bouton TV, autre app, veille) : les relâchements qui
        // suivent ne nous parviendront pas, rien ne doit rester enfoncé sur
        // l'ordinateur. Au retour, l'état réel des manettes est renvoyé.
        // Synchrone : tout est parti avant que l'app soit suspendue.
        for name in [UIApplication.willResignActiveNotification, UIApplication.didEnterBackgroundNotification] {
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                guard let self else { return }
                self.queue.sync { self.appLeft() }
            }
        }
        NotificationCenter.default.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            guard let self else { return }
            self.queue.async { self.appBack() }
        }
        let battery = DispatchSource.makeTimerSource(queue: queue)
        battery.schedule(deadline: .now() + 30, repeating: 30, leeway: .seconds(1))
        battery.setEventHandler { [weak self] in self?.publishPads() }
        battery.resume()
        batteryTimer = battery
    }

    /// Écran de test : l'état en direct est publié 30 fois par seconde tant
    /// qu'il est à l'écran, et seulement alors.
    private func setMonitoring(_ on: Bool) {
        onQueue()
        config.monitoring = on
        if on, liveTimer == nil {
            let timer = DispatchSource.makeTimerSource(queue: queue)
            timer.schedule(deadline: .now(), repeating: 1.0 / 30, leeway: .milliseconds(5))
            timer.setEventHandler { [weak self] in self?.publishLive() }
            timer.resume()
            liveTimer = timer
            liveDirty = true
        } else if !on {
            liveTimer?.cancel()
            liveTimer = nil
        }
        applyHomeGesture()
    }

    private func publishLive() {
        guard config.monitoring, liveDirty else { return }
        liveDirty = false
        let snapshot = Dictionary(uniqueKeysWithValues: slots.values.map { ($0.id, $0.live) })
        DispatchQueue.main.async { self.live = snapshot }
    }

    /// Fil principal : rappel du contrôleur de session.
    private func requestMenu() {
        DispatchQueue.main.async { self.onMenuRequest?() }
    }

    // MARK: - Session

    /// Branche les manettes sur la chaîne d'entrées de la session (fil
    /// principal ; synchrone : la file est prête au retour).
    func attach(_ input: InputPipeline) {
        // Vibrations : reçues sur le fil de kynput, jouées depuis la file.
        input.onRumble = { [weak self] r in
            guard let self else { return }
            self.queue.async { self.rumble(r) }
        }
        // Les manettes s'annoncent une fois les entrées reliées à l'ordinateur :
        // avant, l'annonce se perd, et l'ordinateur rejette ensuite les boutons
        // d'une manette qu'il ne connaît pas. (Une annonce en double est refusée
        // sans dommage ; après une reconnexion, elle est nécessaire.)
        input.onConnected = { [weak self, weak input] in
            guard let self else { return }
            self.queue.async {
                guard let input, self.input === input else { return }
                self.plugAll(input)
            }
        }
        queue.sync {
            self.input = input
            if input.hostSize.width > 0, input.hostSize.height > 0 { hostSize = input.hostSize }
            // `captured` reste au contrôleur de session : l'image redémarre
            // (changement de profil) sans que le menu se ferme.
            for slot in slots.values { slot.motors?.setSession(true) }
            plugAll(input)
            updateMouseTimer()
            applyHomeGesture()
        }
    }

    private func plugAll(_ input: InputPipeline) {
        for slot in orderedSlots where slot.active {
            input.gamepadPlug(id: slot.id, vendor: Self.usbIDs(slot.controller)?.vendor, product: Self.usbIDs(slot.controller)?.product)
        }
    }

    /// Fin de session (fil principal ; synchrone : tout est relâché et
    /// débranché avant que la chaîne d'entrées s'arrête).
    func detach() {
        queue.sync {
            for slot in slots.values {
                slot.motors?.stop()
                slot.motors?.setSession(false)
            }
            if let input { releaseMouse(input) }
            stopMouseTimer()
            if let input {
                for slot in slots.values where slot.active {
                    releaseAll(slot, input: input)
                    input.gamepadUnplug(id: slot.id)
                }
            }
            input = nil
            captured = false
            applyHomeGesture()
        }
    }

    /// Le menu s'ouvre (true) ou se ferme : la manette quitte ou rejoint le jeu.
    /// À l'ouverture, tout est remis à zéro sur l'ordinateur (touches tenues,
    /// six axes, et les clics et touches de la manette en souris) ; à la
    /// fermeture, un stick encore tenu est renvoyé tel quel.
    public func setCaptured(_ value: Bool) {
        queue.sync { capture(value) }
    }

    private func capture(_ value: Bool) {
        onQueue()
        guard value != captured else { return }
        captured = value
        // Menu ouvert : le bouton Home revient à tvOS.
        applyHomeGesture()
        guard let input else { return }
        if value {
            for slot in slots.values { releaseAll(slot, input: input) }
            releaseMouse(input)
        } else {
            resync()
        }
    }

    /// Renvoie l'état réel des sticks et gâchettes : après une remise à zéro,
    /// GameController ne signale que les changements, et un stick tenu
    /// resterait muet. Les touches ne sont pas renvoyées (celle qui vient de
    /// fermer le menu ou de ramener l'app est encore enfoncée, le jeu ne doit
    /// pas la recevoir) ; seul leur état chez nous est remis d'aplomb, pour la
    /// combinaison et l'écran de test.
    private func resync() {
        for slot in slots.values { resync(slot) }
    }

    private func resync(_ slot: Slot) {
        guard input != nil, !captured, !config.padMouse, slot.active else { return }
        guard let pad = slot.controller.extendedGamepad else { return }
        let pressed = Set(Self.buttons(pad).filter { $0.0?.isPressed == true }.map(\.1))
        combo.set(pressed: pressed, for: slot.id)
        slot.live.pressed = Set(pressed.map(\.name))
        liveDirty = true
        axis(slot, .leftX, Self.stick(pad.leftThumbstick.xAxis.value))
        axis(slot, .leftY, Self.stick(-pad.leftThumbstick.yAxis.value))
        axis(slot, .rightX, Self.stick(pad.rightThumbstick.xAxis.value))
        axis(slot, .rightY, Self.stick(-pad.rightThumbstick.yAxis.value))
        axis(slot, .leftTrigger, Self.trigger(pad.leftTrigger.value))
        axis(slot, .rightTrigger, Self.trigger(pad.rightTrigger.value))
    }

    // MARK: - Arrière-plan

    private var inBackground = false

    private func appLeft() {
        guard !inBackground, let input else { return }
        inBackground = true
        KyberLog.app.notice("app en arrière-plan : tout relâché sur l'ordinateur")
        for slot in slots.values { releaseAll(slot, input: input) }
        releaseMouse(input)
        // Clavier Bluetooth et tout le reste : l'ordinateur relâche de son côté.
        input.flushKeys()
    }

    private func appBack() {
        guard inBackground else { return }
        inBackground = false
        resync()
    }

    // MARK: - Branchements

    /// Joueurs dans l'ordre, puis les manettes en attente dans leur ordre d'arrivée.
    private var orderedSlots: [Slot] {
        slots.values.sorted { ($0.active ? $0.player : 100, $0.id) < ($1.active ? $1.player : 100, $1.id) }
    }

    /// XInput n'expose que quatre manettes (ViGEm côté ordinateur) : une
    /// cinquième n'y serait jamais vue et pourrait prendre la place d'une autre.
    private static let maxPlayers = 4

    private func connected(_ c: GCController) {
        onQueue()
        // Ses gestionnaires seront appelés sur la file, plus sur le fil principal.
        c.handlerQueue = queue
        if c.extendedGamepad == nil {
            if c.microGamepad != nil { setUpRemote(c) }
            return
        }
        let key = ObjectIdentifier(c)
        guard slots[key] == nil else { return }
        let used = Set(slots.values.filter(\.active).map(\.player))
        let remembered = config.fixedOrder ? Self.rememberedPlayer(c) : nil
        let player = remembered.flatMap { used.contains($0) || $0 > Self.maxPlayers ? nil : $0 }
            ?? (1...Self.maxPlayers).first { !used.contains($0) } ?? 0
        let slot = Slot(controller: c, id: nextID, player: 0)
        nextID += 1
        slots[key] = slot
        slot.motors = Motors(controller: c)
        slot.motors?.setSession(input != nil)
        setUpPad(slot)
        applyHomeGesture()
        if player > 0 {
            seat(slot, player)
        } else {
            c.playerIndex = .indexUnset
            KyberLog.app.notice("manette en attente : \(c.vendorName ?? "?", privacy: .public) (déjà quatre joueurs)")
            post("\(Self.name(c)) en attente : l'ordinateur ne voit que quatre manettes")
        }
        publishPads()
    }

    /// Donne une place (J1…J4) à une manette et l'annonce à l'ordinateur.
    private func seat(_ slot: Slot, _ player: Int) {
        let c = slot.controller
        slot.player = player
        Self.remember(c, player: player)
        c.playerIndex = GCControllerPlayerIndex(rawValue: player - 1) ?? .indexUnset
        c.light?.color = Self.lightColor(player)
        if let input {
            input.gamepadPlug(id: slot.id, vendor: Self.usbIDs(c)?.vendor, product: Self.usbIDs(c)?.product)
        }
        KyberLog.app.notice("manette branchée : \(c.vendorName ?? "?", privacy: .public) (J\(player))")
    }

    /// Capsule de session (fil principal), effacée au bout de 6 s.
    private func post(_ text: String) {
        DispatchQueue.main.async {
            self.notice = text
            self.noticeClear?.cancel()
            let clear = DispatchWorkItem { [weak self] in self?.notice = nil }
            self.noticeClear = clear
            DispatchQueue.main.asyncAfter(deadline: .now() + 6, execute: clear)
        }
    }

    private func disconnected(_ c: GCController) {
        onQueue()
        if c.extendedGamepad == nil {
            setRemotePresent(GCController.controllers().contains { $0.extendedGamepad == nil && $0.microGamepad != nil && $0 !== c })
            return
        }
        guard let slot = slots.removeValue(forKey: ObjectIdentifier(c)) else { return }
        slot.motors?.stop()
        if slot.active, let input {
            releaseAll(slot, input: input)
            input.gamepadUnplug(id: slot.id)
        }
        // Manette en souris coupée en plein glisser : le clic ne reste pas enfoncé.
        if config.padMouse, !combo.pressed(slot.id).isEmpty, let input { releaseMouse(input) }
        combo.forget(slot.id)
        // Une place se libère : la première manette en attente la prend.
        if slot.active, let next = orderedSlots.first(where: { !$0.active }) {
            seat(next, slot.player)
            resync(next)
            applyHomeGesture()
            post("\(Self.name(next.controller)) rejoint l'ordinateur en J\(next.player)")
        }
        publishPads()
    }

    private func publishPads() {
        onQueue()
        let list = orderedSlots.map { s in
            Pad(id: s.id, name: Self.name(s.controller), kind: Self.kind(s.controller), player: s.player,
                battery: s.controller.battery.map { $0.batteryLevel }, charging: s.controller.battery?.batteryState == .charging,
                rumbles: s.motors != nil)
        }
        DispatchQueue.main.async { if self.pads != list { self.pads = list } }
    }

    private func setRemotePresent(_ on: Bool) {
        guard on != remotePresent else { return }
        remotePresent = on
        DispatchQueue.main.async { self.remoteConnected = on }
    }

    // MARK: - Manettes

    /// Touches d'une manette et leur nom chez kynput ; le pavé tactile des
    /// manettes Sony compte comme Select.
    private static func buttons(_ pad: GCExtendedGamepad) -> [(GCControllerButtonInput?, PadButton)] {
        var buttons: [(GCControllerButtonInput?, PadButton)] = [
            (pad.buttonA, .a), (pad.buttonB, .b), (pad.buttonX, .x), (pad.buttonY, .y),
            (pad.leftShoulder, .leftShoulder), (pad.rightShoulder, .rightShoulder),
            (pad.buttonMenu, .start), (pad.buttonOptions, .back), (pad.buttonHome, .guide),
            (pad.leftThumbstickButton, .leftStick), (pad.rightThumbstickButton, .rightStick),
            (pad.dpad.up, .dpadUp), (pad.dpad.down, .dpadDown), (pad.dpad.left, .dpadLeft), (pad.dpad.right, .dpadRight),
        ]
        if let ds = pad as? GCDualSenseGamepad { buttons.append((ds.touchpadButton, .back)) }
        return buttons
    }

    private func setUpPad(_ slot: Slot) {
        guard let pad = slot.controller.extendedGamepad else { return }
        for (element, button) in Self.buttons(pad) {
            // Gestes du système (capture d'écran sur Options ou Partager) :
            // coupés, la touche part au jeu sans délai. Le bouton Home suit
            // le réglage « Bouton Guide » (`applyHomeGesture`).
            if button != .guide { element?.preferredSystemGestureState = .disabled }
            element?.pressedChangedHandler = { [weak self, weak slot] _, _, pressed in
                guard let self, let slot else { return }
                self.button(slot, button, pressed: pressed)
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

    private func button(_ slot: Slot, _ button: PadButton, pressed: Bool) {
        onQueue()
        // La combinaison ne s'arme qu'en session, si le réglage le permet ; la
        // touche part au jeu comme les autres (formée après 250 ms, voir PadCombo).
        combo.enabled = config.menuFromCombo && input != nil
        combo.button(slot.id, button, pressed: pressed)
        if pressed { slot.live.pressed.insert(button.name) } else { slot.live.pressed.remove(button.name) }
        liveDirty = true
        noteKind(Self.kind(slot.controller))
        guard let input else { return }
        guard !captured else { return }
        if config.padMouse {
            mouseButton(slot, button, pressed: pressed, input: input)
            return
        }
        guard slot.active else { return }
        if button == .guide {
            // Système, ou appui long : le Guide ne vient pas du bouton Home.
            guard config.guideMode == .game else { return }
            if pressed, !guideLogged {
                guideLogged = true
                KyberLog.app.notice("bouton Home reçu par l'app : envoyé au jeu comme Guide")
            }
        }
        if config.guideMode == .longPress, button == .start || button == .back {
            longPress(slot, button, pressed: pressed, input: input)
            return
        }
        input.gamepadButton(id: slot.id, button, pressed: pressed)
    }

    private var guideLogged = false
    private static let guideHold: DispatchTimeInterval = .milliseconds(600)

    /// Réglage « Bouton Guide » sur Appui long : Menu (Start) ou Options
    /// (Select) tenus 600 ms partent au jeu comme Guide, tant qu'on les
    /// tient ; un appui plus court part au relâchement, et son relâchement
    /// 50 ms plus tard (un jeu XInput lit l'état une fois par image : un appui
    /// et un relâchement collés passeraient inaperçus). Le prix : Start et
    /// Select arrivent au jeu au relâchement.
    private func longPress(_ slot: Slot, _ button: PadButton, pressed: Bool, input: InputPipeline) {
        if pressed {
            if let pending = slot.tapRelease.removeValue(forKey: button) {
                pending.cancel()
                input.gamepadButton(id: slot.id, button, pressed: false)
            }
            slot.deferred[button]?.cancel()
            armGuide(slot, button, after: Self.guideHold)
        } else if let item = slot.deferred.removeValue(forKey: button) {
            item.cancel()
            input.gamepadButton(id: slot.id, button, pressed: true)
            let release = DispatchWorkItem { [weak self, weak slot] in
                guard let self, let slot, slot.tapRelease.removeValue(forKey: button) != nil, let input = self.input else { return }
                input.gamepadButton(id: slot.id, button, pressed: false)
            }
            slot.tapRelease[button] = release
            queue.asyncAfter(deadline: .now() + .milliseconds(50), execute: release)
        } else if slot.guideHeld.remove(button) != nil, slot.guideHeld.isEmpty {
            input.gamepadButton(id: slot.id, .guide, pressed: false)
        }
    }

    private func armGuide(_ slot: Slot, _ button: PadButton, after delay: DispatchTimeInterval) {
        let item = DispatchWorkItem { [weak self, weak slot] in
            guard let self, let slot, slot.deferred[button] != nil, let input = self.input, !self.captured else { return }
            // L1 ou R1 tenus : la combinaison du menu est peut-être en train
            // de se former ; pas de Guide au jeu juste avant le menu.
            if !self.combo.pressed(slot.id).isDisjoint(with: [.leftShoulder, .rightShoulder]) {
                self.armGuide(slot, button, after: .milliseconds(100))
                return
            }
            slot.deferred[button] = nil
            if slot.guideHeld.isEmpty { input.gamepadButton(id: slot.id, .guide, pressed: true) }
            slot.guideHeld.insert(button)
        }
        slot.deferred[button] = item
        queue.asyncAfter(deadline: .now() + delay, execute: item)
    }

    /// Oublie les appuis longs en cours (combinaison formée, menu, réglage
    /// changé) et relâche le Guide qu'ils tenaient.
    private func dropLongPress(_ slot: Slot, input: InputPipeline?) {
        slot.deferred.values.forEach { $0.cancel() }
        slot.deferred.removeAll()
        for (button, release) in slot.tapRelease {
            release.cancel()
            if slot.active { input?.gamepadButton(id: slot.id, button, pressed: false) }
        }
        slot.tapRelease.removeAll()
        if !slot.guideHeld.isEmpty, slot.active { input?.gamepadButton(id: slot.id, .guide, pressed: false) }
        slot.guideHeld.removeAll()
    }

    private func setGuideMode(_ mode: GuideMode) {
        onQueue()
        guard mode != config.guideMode else { return }
        config.guideMode = mode
        for slot in slots.values { dropLongPress(slot, input: input) }
        applyHomeGesture()
    }

    /// Bouton Home au jeu : demandé à tvOS pendant la session (menu fermé) et
    /// sur l'écran de test, avec le réglage Jeu. `.disabled` et non
    /// `.alwaysReceive` : ce dernier (déprécié avec tvOS 27) laisse aussi
    /// tvOS agir, qui renverrait à l'accueil. tvOS peut refuser : l'écran de
    /// test le montre (Home allumé, ou retour à l'accueil).
    private func applyHomeGesture() {
        let toGame = config.guideMode == .game && ((input != nil && !captured) || config.monitoring)
        let state: GCControllerElement.SystemGestureState = toGame ? .disabled : .enabled
        for slot in slots.values {
            // Manette en attente : rien ne part au jeu, Home reste à tvOS.
            let wanted = slot.active ? state : .enabled
            guard let home = slot.controller.extendedGamepad?.buttonHome, home.preferredSystemGestureState != wanted else { continue }
            home.preferredSystemGestureState = wanted
        }
    }

    /// Combinaison formée (quatre touches tenues 250 ms, voir PadCombo) : la
    /// suite est au contrôleur de session : prendre la manette (les touches
    /// tenues sont alors relâchées pour l'ordinateur), ou la rendre.
    private func comboFormed(_ id: UInt32) {
        onQueue()
        guard config.menuFromCombo, input != nil, let slot = slots.values.first(where: { $0.id == id }) else { return }
        KyberLog.app.notice("combinaison L1 + R1 + Start + Select")
        // Appui long sur Start ou Select en cours : il n'aboutit pas.
        dropLongPress(slot, input: input)
        requestMenu()
    }

    /// Les glyphes de l'interface suivent la dernière manette utilisée.
    private func noteKind(_ kind: Kind) {
        guard kind != publishedKind else { return }
        publishedKind = kind
        DispatchQueue.main.async { self.lastKind = kind }
    }

    /// Axe d'une manette qui joue : InputPipeline le filtre (au 1/512 de la
    /// course), regroupe par tranche de 4 ms et réessaie les retours au repos.
    private func axis(_ slot: Slot, _ axis: PadAxis, _ value: Int16) {
        onQueue()
        liveDirty = true
        guard let input, !captured, !config.padMouse, slot.active else { return }
        input.gamepadAxis(id: slot.id, axis, value: value)
    }

    // MARK: - Manette en souris

    /// Clics et touches tenus par la manette en souris et la télécommande
    /// (politique « tout relâcher » partagée : menu, arrière-plan, fin de session).
    private let held = ReleaseAll()
    private var altHeld = false
    private var altRelease: DispatchWorkItem?
    private var mouseTimer: DispatchSourceTimer?
    private var moveRest = (x: 0.0, y: 0.0)
    private var wheelRest = (x: 0.0, y: 0.0)

    // Usages HID (page 7) des touches envoyées.
    private static let keyUp: UInt16 = 0x52, keyDown: UInt16 = 0x51, keyLeft: UInt16 = 0x50, keyRight: UInt16 = 0x4F
    private static let keyTab: UInt16 = 0x2B, keyEscape: UInt16 = 0x29
    private static let keyLeftShift: UInt16 = 0xE1, keyLeftAlt: UInt16 = 0xE2, keyLeftGUI: UInt16 = 0xE3

    private func setPadMouse(_ on: Bool) {
        onQueue()
        guard on != config.padMouse else { return }
        config.padMouse = on
        if let input {
            if on {
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
        held.key(scancode: scancode, pressed: pressed)
        input.key(scancode: scancode, pressed: pressed)
    }

    private func tap(_ usage: UInt16, input: InputPipeline) {
        key(usage, true, input: input)
        key(usage, false, input: input)
    }

    /// Clic de la manette en souris ou de la télécommande (suivi, pour être
    /// relâché au menu, en arrière-plan ou en fin de session). Les
    /// déplacements partent de cette même file (PointerLayer) : un clic reste
    /// derrière le déplacement qui le précède.
    private func click(_ b: InputPipeline.MouseButton, _ pressed: Bool, input: InputPipeline) {
        held.mouseButton(b, pressed: pressed)
        input.mouseButton(b, pressed: pressed)
    }

    private func mouseButton(_ slot: Slot, _ button: PadButton, pressed: Bool, input: InputPipeline) {
        func click(_ b: InputPipeline.MouseButton) { self.click(b, pressed, input: input) }
        // Touches de la combinaison du menu : elles agissent au relâchement,
        // sauf si la combinaison a été formée entre-temps.
        let released = !pressed && !combo.formed(slot.id)
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
        queue.asyncAfter(deadline: .now() + 0.9, execute: release)
    }

    /// Position absolue par la couche de pointeur ; à défaut, déplacement
    /// relatif (sous-pixels gardés d'un appel à l'autre).
    private func move(dx: Double, dy: Double, input: InputPipeline) {
        if let pointer {
            pointer.move(dx: dx, dy: dy)
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

    /// Relâche clics et touches de la manette en souris (boutons d'abord, puis
    /// touches, Alt compris).
    private func releaseMouse(_ input: InputPipeline) {
        altRelease?.cancel()
        altHeld = false
        held.release(into: input)
    }

    private func updateMouseTimer() {
        guard config.padMouse, input != nil else {
            stopMouseTimer()
            return
        }
        guard mouseTimer == nil else { return }
        let timer = DispatchSource.makeTimerSource(queue: queue)
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
        guard config.padMouse, !captured, let input else { return }
        let dt = 1.0 / 120
        var dx = 0.0, dy = 0.0, wx = 0.0, wy = 0.0
        for slot in slots.values {
            let left = slot.live.leftStick
            let m = Double(hypot(left.x, left.y))
            if m > 0.12 {
                let n = min(1, (m - 0.12) / 0.88)
                let precision = 1 - 0.7 * Double(slot.live.leftTrigger)
                let boost = 1 + 1.5 * Double(slot.live.rightTrigger)
                let speed = Self.fullSpeed(hostSize) * Self.curve(n) * config.padMouseSpeed * precision * boost
                dx += Double(left.x) / m * speed * dt
                dy -= Double(left.y) / m * speed * dt
            }
            let right = slot.live.rightStick
            // Jusqu'à 14 crans de molette par seconde ; stick vers le haut =
            // défilement vers le haut (kynput : dy positif = vers le bas).
            if abs(right.y) > 0.15 { wy -= Double(right.y) * 14 * dt }
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

    /// Courbe d'accélération du pointeur (manette et Siri Remote) : lent près
    /// de zéro, pour viser, rapide à fond.
    private static func curve(_ n: Double) -> Double { pow(min(1, max(0, n)), 2.2) }

    /// Pointeur à fond (pixels par seconde) : l'écran de l'ordinateur traversé
    /// en 1,5 s, quelle que soit sa définition (2 560 px/s en 4K).
    private static func fullSpeed(_ host: CGSize) -> Double { Double(host.width) / 1.5 }

    private func releaseAll(_ slot: Slot, input: InputPipeline) {
        guard slot.active else { return }
        dropLongPress(slot, input: input)
        for b in combo.pressed(slot.id) { input.gamepadButton(id: slot.id, b, pressed: false) }
        input.gamepadAxesRest(id: slot.id)
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
        setRemotePresent(true)
        guard let micro = c.microGamepad else { return }
        micro.reportsAbsoluteDpadValues = true
        micro.allowsRotation = false
        micro.buttonMenu.pressedChangedHandler = { [weak self] _, _, pressed in
            guard let self, pressed, self.config.menuFromRemote, self.input != nil, !self.captured else { return }
            self.requestMenu()
        }
        // Doigt sur le pavé : bouton « surface tactile » de la Siri Remote
        // (2ᵉ génération et suivantes), dont l'état `touched` dit si le doigt
        // est posé, centre du pavé compris. Sans lui (1ʳᵉ génération), (0, 0)
        // reste le seul signe d'un doigt levé.
        let surface = c.physicalInputProfile.buttons[GCInputDirectionalTouchSurfaceButton]
        surface?.touchedChangedHandler = { [weak self] _, _, _, touched in
            if !touched { self?.lastTouch = nil }
        }
        micro.dpad.valueChangedHandler = { [weak self, weak surface] _, x, y in
            guard let self, self.config.remoteMouse, !self.captured, let input = self.input else { return }
            self.remoteMoved(x: x, y: y, touched: surface?.isTouched, input: input)
        }
        micro.buttonA.pressedChangedHandler = { [weak self] _, _, pressed in
            guard let self, self.config.remoteMouse, !self.captured, let input = self.input else { return }
            self.click(.left, pressed, input: input)
        }
        micro.buttonX.pressedChangedHandler = { [weak self] _, _, pressed in
            guard let self, self.config.remoteMouse, !self.captured, let input = self.input else { return }
            self.click(.right, pressed, input: input)
        }
    }

    /// Pavé de la Siri Remote en souris (positions absolues de -1 à 1) : le
    /// déplacement du doigt devient un déplacement du pointeur, avec la courbe
    /// de la manette en souris appliquée à la vitesse du doigt, et rapporté à
    /// l'écran de l'ordinateur (avant : 900 px par demi-pavé quelle que soit
    /// la définition, lent en 4K).
    private func remoteMoved(x: Float, y: Float, touched: Bool?, input: InputPipeline) {
        let now = ProcessInfo.processInfo.systemUptime
        let lifted = touched.map { !$0 } ?? (x == 0 && y == 0)
        guard !lifted else {
            lastTouch = nil
            return
        }
        let point = CGPoint(x: CGFloat(x), y: CGFloat(y))
        defer {
            lastTouch = point
            lastTouchTime = now
        }
        guard let last = lastTouch else { return }
        let dx = Double(point.x - last.x), dy = Double(last.y - point.y)
        let distance = hypot(dx, dy)
        // Saut (doigt reposé ailleurs, échantillon parasite au lever) : on
        // repart de là sans bouger.
        guard distance > 0, distance < 0.6 else { return }
        let dt = min(0.05, max(0.004, now - lastTouchTime))
        // Vitesse du doigt en pavés (2 unités) par seconde ; pleine vitesse
        // vers 2,5 pavés par seconde (balayage rapide).
        let n = distance / 2 / dt / 2.5
        // Pixels par unité de pavé : doigt lent, un pavé = 40 % de l'écran
        // (pour viser) ; vite, la courbe en ajoute jusqu'à 2,4 écrans de plus.
        let gain = Double(hostSize.width) * (0.2 + 1.2 * Self.curve(n))
        move(dx: dx * gain, dy: dy * gain, input: input)
    }

    // MARK: - Vibrations

    private func rumble(_ r: GamepadRumble) {
        onQueue()
        // Fin de session : un paquet en retard ne relance pas le moteur.
        guard input != nil, config.rumbleEnabled, let slot = slots.values.first(where: { $0.id == r.id }) else { return }
        slot.motors?.play(large: r.largeMotor, small: r.smallMotor, durationMs: r.durationMs)
    }

    /// Vibration d'essai (écran des manettes).
    public func testRumble(_ id: UInt32) {
        queue.async {
            guard let slot = self.slots.values.first(where: { $0.id == id }) else { return }
            slot.motors?.play(large: 52000, small: 30000, durationMs: 450)
        }
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
/// sinon un seul. Chaque moteur joue en boucle un motif continu dont
/// l'intensité suit chaque paquet `Rumble` (`hapticIntensityControl`) : plus
/// de lecteur recréé à chaque paquet, qui hachait la vibration (effet
/// « mitraillette ») et coûtait plusieurs millisecondes. Le moteur Core
/// Haptics reste démarré pendant la session ; la vibration ne s'arrête vraiment
/// que sur (0, 0), au bout de la durée demandée, ou en fin de session.
///
/// Les appels à Core Haptics sont synchrones vers le service haptique (le
/// premier démarrage du moteur peut prendre des dizaines de millisecondes) :
/// ils ont leur propre file, pour ne jamais retarder une touche sur la file
/// des entrées. Les méthodes publiques y passent elles-mêmes.
private final class Motors {
    static let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.vibrations", qos: .userInitiated)
    private var large: Motor?
    private var small: Motor?
    private let queue: DispatchQueue
    private var expiry: DispatchWorkItem?
    /// En session, le moteur Core Haptics reste démarré entre deux vibrations ;
    /// hors session (vibration d'essai), il s'arrête après.
    private var session = false

    func setSession(_ on: Bool) {
        queue.async {
            self.session = on
            if !on { self.shutdown() }
        }
    }

    init?(controller: GCController) {
        guard let haptics = controller.haptics else { return nil }
        let queue = Self.queue
        self.queue = queue
        let localities = haptics.supportedLocalities
        if localities.contains(.leftHandle), localities.contains(.rightHandle) {
            // Gros moteur à gauche (grave), petit à droite (aigu), comme XInput.
            large = Motor(haptics.createEngine(withLocality: .leftHandle), sharpness: 0.2, queue: queue)
            small = Motor(haptics.createEngine(withLocality: .rightHandle), sharpness: 0.8, queue: queue)
        } else {
            large = Motor(haptics.createEngine(withLocality: .default), sharpness: 0.4, queue: queue)
        }
        if large == nil { return nil }
    }

    /// Paquet `Rumble` : intensités de 0 à 65535 ; durée en millisecondes,
    /// -1 (ou 0) : jusqu'à nouvel ordre, 5 s au plus faute de (0, 0), comme
    /// kynput avec SDL sur Mac.
    func play(large l: UInt16, small s: UInt16, durationMs: Int) {
        queue.async { self.apply(large: l, small: s, durationMs: durationMs) }
    }

    private func apply(large l: UInt16, small s: UInt16, durationMs: Int) {
        expiry?.cancel()
        expiry = nil
        let li = Float(l) / 65535, si = Float(s) / 65535
        if let small {
            large?.set(li)
            small.set(si)
        } else {
            large?.set(max(li, si))
        }
        guard l > 0 || s > 0 else {
            if !session { shutdown() }
            return
        }
        let seconds = durationMs > 0 ? Double(durationMs) / 1000 : 5
        let item = DispatchWorkItem { [weak self] in self?.apply(large: 0, small: 0, durationMs: 0) }
        expiry = item
        queue.asyncAfter(deadline: .now() + seconds, execute: item)
    }

    /// Fin de session ou manette débranchée : vibration et moteur arrêtés.
    func stop() {
        queue.async {
            self.expiry?.cancel()
            self.expiry = nil
            self.shutdown()
        }
    }

    private func shutdown() {
        large?.shutdown()
        small?.shutdown()
    }

    private final class Motor {
        private let engine: CHHapticEngine
        private let sharpness: Float
        private let queue: DispatchQueue
        private var player: CHHapticAdvancedPatternPlayer?
        private var running = false
        private var playing = false
        private var level: Float = 0
        /// Durée du motif joué en boucle (le maximum d'un événement continu).
        private static let span: TimeInterval = 30

        init?(_ engine: CHHapticEngine?, sharpness: Float, queue: DispatchQueue) {
            guard let engine else { return nil }
            self.engine = engine
            self.sharpness = sharpness
            self.queue = queue
            engine.isAutoShutdownEnabled = false
            engine.playsHapticsOnly = true
            // Rappels de Core Haptics sur un fil à lui : on repasse sur la file.
            engine.stoppedHandler = { [weak self] reason in
                guard let self else { return }
                queue.async {
                    self.running = false
                    self.player = nil
                    self.playing = false
                    KyberLog.app.notice("vibration : moteur arrêté par le système (\(reason.rawValue))")
                }
            }
            // Serveur haptique relancé : les lecteurs ne valent plus rien ; la
            // vibration en cours reprend.
            engine.resetHandler = { [weak self] in
                guard let self else { return }
                queue.async {
                    self.running = false
                    self.player = nil
                    self.playing = false
                    if self.level > 0 { self.set(self.level) }
                }
            }
        }

        /// Règle l'intensité (0…1) : 0 arrête le lecteur, le moteur reste démarré.
        func set(_ value: Float) {
            let v = min(1, max(0, value))
            level = v
            guard v > 0.01 else {
                if playing {
                    try? player?.stop(atTime: CHHapticTimeImmediate)
                    playing = false
                }
                return
            }
            do {
                if !running {
                    try engine.start()
                    running = true
                }
                let p = try player ?? makePlayer()
                player = p
                let intensity = [CHHapticDynamicParameter(parameterID: .hapticIntensityControl, value: v, relativeTime: 0)]
                if !playing {
                    // Intensité donnée avant le départ si Core Haptics la
                    // garde : pas d'à-coup à pleine intensité.
                    try? p.sendParameters(intensity, atTime: CHHapticTimeImmediate)
                    try p.start(atTime: CHHapticTimeImmediate)
                    playing = true
                }
                try p.sendParameters(intensity, atTime: CHHapticTimeImmediate)
            } catch {
                KyberLog.app.error("vibration : \(error.localizedDescription, privacy: .public)")
                // Un lecteur qui tourne encore ne doit pas rester à pleine
                // intensité sans personne pour l'arrêter.
                try? player?.stop(atTime: CHHapticTimeImmediate)
                player = nil
                playing = false
                running = false
            }
        }

        /// Motif continu d'intensité 1, en boucle ; l'intensité réelle vient
        /// du paramètre dynamique.
        private func makePlayer() throws -> CHHapticAdvancedPatternPlayer {
            let event = CHHapticEvent(eventType: .hapticContinuous, parameters: [
                CHHapticEventParameter(parameterID: .hapticIntensity, value: 1),
                CHHapticEventParameter(parameterID: .hapticSharpness, value: sharpness),
            ], relativeTime: 0, duration: Self.span)
            let p = try engine.makeAdvancedPlayer(with: CHHapticPattern(events: [event], parameters: []))
            p.loopEnabled = true
            p.loopEnd = Self.span
            return p
        }

        func shutdown() {
            if playing { try? player?.stop(atTime: CHHapticTimeImmediate) }
            player = nil
            playing = false
            level = 0
            if running {
                engine.stop(completionHandler: nil)
                running = false
            }
        }
    }
}
