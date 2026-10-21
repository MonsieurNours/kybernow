// Entrées vers l'ordinateur (libkynput) et curseur venant de lui.
//
// Le pipeline client de kynput produit déjà certaines entrées sur macOS
// (manettes via SDL) ; on produit les autres depuis les événements AppKit,
// comme le client Kyber. Les annonces de curseur du PC (forme, position,
// visibilité) arrivent par un gestionnaire dédié et vont à la couche de
// pointeur partagée (PointerLayer), sur sa file.
//
// Sur l'Apple TV, kynput ne produit rien lui-même : manettes, clavier et
// souris viennent de GameController (apps/tvos/KyberKitTV), sans presse-papiers
// (tvOS n'en a pas : `Pasteboard.general` y est nil).

import CoreGraphics
import Foundation
@_implementationOnly import CKyber

/// Pointeur de l'ordinateur, tel qu'il l'annonce.
public struct RemoteCursor {
    public let visible: Bool
    /// Forme, en pixels de l'écran de l'ordinateur ; nil : inchangée ou vide.
    public let image: CGImage?
    public let hotSpot: CGPoint
    /// Position sur l'écran de l'ordinateur, en pixels.
    public let position: CGPoint
}

private let cursorTypes: UnsafeMutablePointer<kynput_InputType> = {
    let p = UnsafeMutablePointer<kynput_InputType>.allocate(capacity: 1)
    p.initialize(to: KYNPUT_INPUT_TYPE_CURSOR)
    return p
}()

private let clipboardTypes: UnsafeMutablePointer<kynput_InputType> = {
    let p = UnsafeMutablePointer<kynput_InputType>.allocate(capacity: 1)
    p.initialize(to: KYNPUT_INPUT_TYPE_CLIPBOARD)
    return p
}()

private let gamepadEventTypes: UnsafeMutablePointer<kynput_InputType> = {
    let p = UnsafeMutablePointer<kynput_InputType>.allocate(capacity: 1)
    p.initialize(to: KYNPUT_INPUT_TYPE_GAMEPAD_EVENT)
    return p
}()

/// Vibration demandée par le jeu pour une manette (moteurs de 0 à 65535,
/// durée en millisecondes, -1 : jusqu'à nouvel ordre).
public struct GamepadRumble: Sendable {
    public let id: UInt32
    public let largeMotor: UInt16
    public let smallMotor: UInt16
    public let durationMs: Int
}

/// Touches et axes de manette, dans le vocabulaire de kynput (celui de SDL).
public enum PadButton: Sendable, CaseIterable {
    case a, b, x, y, back, guide, start, leftStick, rightStick, leftShoulder, rightShoulder
    case dpadUp, dpadDown, dpadLeft, dpadRight

    var sdk: kynput_GamepadButton {
        switch self {
        case .a: return KYNPUT_GAMEPAD_BUTTON_A
        case .b: return KYNPUT_GAMEPAD_BUTTON_B
        case .x: return KYNPUT_GAMEPAD_BUTTON_X
        case .y: return KYNPUT_GAMEPAD_BUTTON_Y
        case .back: return KYNPUT_GAMEPAD_BUTTON_BACK
        case .guide: return KYNPUT_GAMEPAD_BUTTON_GUIDE
        case .start: return KYNPUT_GAMEPAD_BUTTON_START
        case .leftStick: return KYNPUT_GAMEPAD_BUTTON_LEFT_STICK
        case .rightStick: return KYNPUT_GAMEPAD_BUTTON_RIGHT_STICK
        case .leftShoulder: return KYNPUT_GAMEPAD_BUTTON_LEFT_SHOULDER
        case .rightShoulder: return KYNPUT_GAMEPAD_BUTTON_RIGHT_SHOULDER
        case .dpadUp: return KYNPUT_GAMEPAD_BUTTON_D_PAD_UP
        case .dpadDown: return KYNPUT_GAMEPAD_BUTTON_D_PAD_DOWN
        case .dpadLeft: return KYNPUT_GAMEPAD_BUTTON_D_PAD_LEFT
        case .dpadRight: return KYNPUT_GAMEPAD_BUTTON_D_PAD_RIGHT
        }
    }

    /// Touche d'un paquet kynput (manettes SDL du Mac).
    init?(sdk: kynput_GamepadButton) {
        guard let button = Self.allCases.first(where: { $0.sdk == sdk }) else { return nil }
        self = button
    }
}

public enum PadAxis: Sendable {
    case leftX, leftY, rightX, rightY, leftTrigger, rightTrigger

    var sdk: kynput_GamepadAxis {
        switch self {
        case .leftX: return KYNPUT_GAMEPAD_AXIS_LEFT_X
        case .leftY: return KYNPUT_GAMEPAD_AXIS_LEFT_Y
        case .rightX: return KYNPUT_GAMEPAD_AXIS_RIGHT_X
        case .rightY: return KYNPUT_GAMEPAD_AXIS_RIGHT_Y
        case .leftTrigger: return KYNPUT_GAMEPAD_AXIS_TRIGGER_LEFT
        case .rightTrigger: return KYNPUT_GAMEPAD_AXIS_TRIGGER_RIGHT
        }
    }
}

/// Reçoit les vibrations envoyées par l'ordinateur. Appelé sur le fil de
/// kynput (Apple TV : le pont des manettes repasse sur sa propre file, sans
/// détour par le fil principal) ; le rappel est réglé du fil principal.
private final class RumbleSink {
    private let lock = NSLock()
    private var handler: ((GamepadRumble) -> Void)?
    var onRumble: ((GamepadRumble) -> Void)? {
        get { lock.lock(); defer { lock.unlock() }; return handler }
        set { lock.lock(); handler = newValue; lock.unlock() }
    }
}

/// Presse-papiers partagé et la file où ses paquets sont traités.
private final class ClipboardSink {
    let bridge: ClipboardBridge
    let queue: DispatchQueue
    init(bridge: ClipboardBridge, queue: DispatchQueue) {
        self.bridge = bridge
        self.queue = queue
    }
}

private final class CursorSink {
    weak var pointer: PointerLayer?
    /// Empreinte de la dernière forme reçue : un simple déplacement renvoie
    /// la même image (kyber-patches serveur 0006), inutile de la refaire.
    var lastShape: Int?
    init(pointer: PointerLayer) { self.pointer = pointer }
}

final class InputPipeline {
    let pipeline: OpaquePointer
    private let sink: OpaquePointer
    let displayID: UInt32
    /// Taille de l'écran de l'ordinateur au départ de l'image, en pixels
    /// (Apple TV : vitesse du pointeur rapportée à l'écran).
    let hostSize: CGSize
    /// Combinaison L1 + R1 + START + SELECT (manettes lues par kynput via SDL).
    let gamepad = GamepadGate()
    /// Presse-papiers partagé (nil : désactivé dans les réglages, ou plateforme sans presse-papiers).
    private(set) var clipboard: ClipboardBridge?
    /// Vibrations des manettes (Apple TV : kynput ne les gère pas lui-même).
    var onRumble: ((GamepadRumble) -> Void)? {
        get { rumble.onRumble }
        set { rumble.onRumble = newValue }
    }
    private let rumble = RumbleSink()

    /// `pointer` : la couche de pointeur de la vue de session, qui reçoit les
    /// annonces de l'ordinateur sur sa file.
    init(display: KyberDisplay, pointer: PointerLayer, clipboard shareClipboard: Bool) {
        displayID = display.id
        hostSize = CGSize(width: display.width, height: display.height)
        pipeline = kynput_pipeline_new(kynput_get_client_config())
        sink = kynput_pipeline_get_input_sink(pipeline)
        packets = Self.makePackets(sink: sink, queue: queue)

        // Pointeur de l'ordinateur.
        let box = Unmanaged.passRetained(CursorSink(pointer: pointer)).toOpaque()
        let cursor = kynput_handler_new(kynput_HandlerInterface(
            start: nil,
            stop: nil,
            handled_types: cursorTypes,
            handled_types_count: 1,
            consume: { packet, userdata in
                guard let packet, let userdata, let c = kynput_cursor_from_packet(packet) else { return }
                defer { kynput_cursor_unref(c) }
                let box = Unmanaged<CursorSink>.fromOpaque(userdata).takeUnretainedValue()
                let remote = InputPipeline.decode(c, lastShape: &box.lastShape)
                guard let pointer = box.pointer else { return }
                pointer.queue.async { pointer.remoteCursorChanged(remote) }
            },
            produced_types: nil,
            produced_types_count: 0,
            query_type_metadata: { type, _ in
                type == KYNPUT_INPUT_TYPE_CURSOR ? kynput_input_type_metadata_cursor_pack(KYNPUT_CURSOR_FORMAT_RAW) : nil
            },
            destroy: { userdata in
                if let userdata { Unmanaged<CursorSink>.fromOpaque(userdata).release() }
            },
            userdata: box
        ))
        kynput_pipeline_plug_handler(pipeline, cursor)
        kynput_handler_unref(cursor)

        // Presse-papiers : reçoit les paquets de l'ordinateur, produit les nôtres.
        // Le routeur appelle `consume` sous son verrou : le paquet est copié et
        // traité sur notre file, d'où la réponse repart sans réentrer dans le
        // routeur (ClipboardBridge.received → send → consume).
        if shareClipboard, let board = Pasteboard.general {
            let bridge = ClipboardBridge(board: board)
            let sink = ClipboardSink(bridge: bridge, queue: queue)
            let handler = kynput_handler_new(kynput_HandlerInterface(
                start: nil,
                stop: nil,
                handled_types: clipboardTypes,
                handled_types_count: 1,
                consume: { packet, userdata in
                    guard let packet, let userdata, let copy = kynput_packet_clone(packet) else { return }
                    let sink = Unmanaged<ClipboardSink>.fromOpaque(userdata).takeUnretainedValue()
                    sink.queue.async {
                        sink.bridge.received(copy)
                        kynput_packet_unref(copy)
                    }
                },
                produced_types: clipboardTypes,
                produced_types_count: 1,
                query_type_metadata: nil,
                destroy: { userdata in
                    if let userdata { Unmanaged<ClipboardSink>.fromOpaque(userdata).release() }
                },
                userdata: Unmanaged.passRetained(sink).toOpaque()
            ))
            kynput_pipeline_plug_handler(pipeline, handler)
            kynput_handler_unref(handler)
            clipboard = bridge
        }
        #if !os(macOS)
        // Vibrations : l'ordinateur les envoie, GameController les joue.
        let rumbleHandler = kynput_handler_new(kynput_HandlerInterface(
            start: nil,
            stop: nil,
            handled_types: gamepadEventTypes,
            handled_types_count: 1,
            consume: { packet, userdata in
                guard let packet, let userdata else { return }
                var r = kynput_GamepadEventRumble()
                guard kynput_gamepad_rumble_from_packet(packet, &r) else { return }
                let sink = Unmanaged<RumbleSink>.fromOpaque(userdata).takeUnretainedValue()
                let rumble = GamepadRumble(id: r.id, largeMotor: r.large_motor, smallMotor: r.small_motor, durationMs: Int(r.duration))
                sink.onRumble?(rumble)
            },
            produced_types: nil,
            produced_types_count: 0,
            query_type_metadata: nil,
            destroy: { userdata in
                if let userdata { Unmanaged<RumbleSink>.fromOpaque(userdata).release() }
            },
            userdata: Unmanaged.passRetained(rumble).toOpaque()
        ))
        kynput_pipeline_plug_handler(pipeline, rumbleHandler)
        kynput_handler_unref(rumbleHandler)
        #endif

        // Entrées que le pipeline ne produit pas lui-même : déclarées ici,
        // envoyées par StreamView.
        var wanted = [KYNPUT_INPUT_TYPE_KEYBOARD, KYNPUT_INPUT_TYPE_MOUSE_POSITION, KYNPUT_INPUT_TYPE_MOUSE_MOVE, KYNPUT_INPUT_TYPE_MOUSE_BUTTON, KYNPUT_INPUT_TYPE_MOUSE_WHEEL]
        #if !os(macOS)
        wanted += [KYNPUT_INPUT_TYPE_GAMEPAD_CONTROL, KYNPUT_INPUT_TYPE_GAMEPAD_EVENT, KYNPUT_INPUT_TYPE_UNICODE]
        #endif
        let missing = wanted.filter { !kynput_pipeline_produces_type(pipeline, $0) }
        KyberLog.app.notice("entrées : \(wanted.count - missing.count) types produits par kynput, \(missing.count) par l'app (écran \(display.id))")
        if !missing.isEmpty {
            // Le tableau doit vivre aussi longtemps que le gestionnaire : on le garde.
            let types = UnsafeMutablePointer<kynput_InputType>.allocate(capacity: missing.count)
            types.initialize(from: missing, count: missing.count)
            let producer = kynput_handler_new(kynput_HandlerInterface(
                start: nil, stop: nil,
                handled_types: nil, handled_types_count: 0, consume: nil,
                produced_types: types, produced_types_count: missing.count,
                query_type_metadata: nil, destroy: nil, userdata: nil
            ))
            kynput_pipeline_plug_handler(pipeline, producer)
            kynput_handler_unref(producer)
        }

        #if os(macOS)
        // Manettes : chaque paquet passe par la combinaison avant de partir.
        // (Apple TV : la combinaison est vue par GameController, avant l'envoi.)
        let gate = Unmanaged.passRetained(gamepad).toOpaque()
        var hook = kynput_RouterHook(
            on_packet: { packet, userdata in
                guard let packet, let userdata else { return kynput_RouterHookPacketResult(process_event: true) }
                let gate = Unmanaged<GamepadGate>.fromOpaque(userdata).takeUnretainedValue()
                return kynput_RouterHookPacketResult(process_event: gate.filter(packet))
            },
            on_control_event: nil,
            on_destroy: { userdata in
                if let userdata { Unmanaged<GamepadGate>.fromOpaque(userdata).release() }
            },
            userdata: gate
        )
        kynput_pipeline_set_hook(pipeline, &hook)
        gamepad.release = { [weak self] buttons, axes in
            guard let self else { return }
            let gate = self.gamepad
            // Chaque essai réarme son autorisation : le hook la consomme avant
            // que la file d'envoi se prononce.
            for (id, button) in buttons {
                self.sendReliably("manette") {
                    gate.allow(id: id, button: button)
                    var b = kynput_GamepadEventButton(id: id, type_: button, pressed: false)
                    return kynput_gamepad_button_to_packet(KYNPUT_INPUT_TARGET_HOST, &b)
                }
            }
            for (id, axis) in axes {
                self.sendReliably("axe de manette") {
                    gate.allow(id: id, axis: axis)
                    var a = kynput_GamepadEventAxis(id: id, type_: axis, value: 0)
                    return kynput_gamepad_axis_to_packet(KYNPUT_INPUT_TARGET_HOST, &a)
                }
            }
        }
        #endif

        // Souris absolue au départ : le curseur du PC suit celui du Mac.
        setRelative(false)
    }

    /// Souris relative (jeux) : seuls les déplacements partent.
    func setRelative(_ on: Bool) {
        lock.lock()
        relativeOn = on
        lock.unlock()
        kynput_pipeline_set_type_processing_state(pipeline, KYNPUT_INPUT_TYPE_MOUSE_MOVE, kynput_router_processing_state_bool(on))
    }

    /// Souris relative active : lisible de n'importe quel fil (Apple TV : la
    /// souris Bluetooth envoie ses déplacements bruts depuis la file des
    /// entrées, sans passer par le fil principal).
    var isRelative: Bool {
        lock.lock()
        defer { lock.unlock() }
        return relativeOn
    }
    private var relativeOn = false

    deinit {
        kynput_consumer_unref(sink)
        kynput_pipeline_unref(pipeline)
    }

    func stop() {
        clipboard?.stop()
        lock.lock()
        pendingAxes.removeAll()
        lock.unlock()
        let dropped = packets.stop()
        if dropped > 0 {
            KyberLog.app.notice("entrées : \(dropped) paquets perdus pendant la session (file d'envoi pleine ou réseau absent)")
        }
        kynput_pipeline_flush_keys(pipeline)
    }

    /// Arrête les gestionnaires de kynput (manettes SDL) ; sans effet s'ils le sont déjà.
    func shutdown() {
        kynput_pipeline_stop(pipeline)
    }

    /// À appeler une fois le pipeline branché au réseau.
    func startClipboard() {
        guard let clipboard else { return }
        clipboard.send = { [weak self] packet in self?.send(packet, "presse-papiers") }
        clipboard.start()
    }

    // MARK: - Envoi

    // Tout ce que l'app produit passe par PacketQueue : envoi à kynput, refus
    // comptés, file de secours pour les paquets qui ne se rattrapent pas. Les
    // manettes lues par SDL (Mac) n'y passent pas : kynput les envoie
    // lui-même, GamepadGate en réduit seulement le débit.

    /// Paquets refusés par kynput, jamais arrivés.
    var droppedPackets: Int { packets.droppedPackets }
    private let lock = NSLock()
    /// File série des réessais, des axes regroupés et du presse-papiers.
    private let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.entrees")
    private let packets: PacketQueue

    private static func makePackets(sink: OpaquePointer, queue: DispatchQueue) -> PacketQueue {
        let q = PacketQueue(queue: queue) { kynput_consumer_consume(sink, $0) }
        q.onFirst = { kind, rc in KyberLog.app.notice("entrées : premier envoi \(kind, privacy: .public) (code \(rc))") }
        q.onDropped = { kind, final, total in
            if final {
                KyberLog.app.error("entrées : \(kind, privacy: .public) perdu après 200 ms de réessais (\(total) perdus)")
            } else {
                KyberLog.app.notice("entrées : \(kind, privacy: .public) refusé par la file d'envoi (\(total) perdus)")
            }
        }
        return q
    }

    /// Paquet qui se rattrape tout seul au suivant : envoyé une fois, perdu
    /// s'il est refusé (voir PacketQueue). Appelable de tout fil.
    @discardableResult
    private func send(_ packet: OpaquePointer?, _ kind: String) -> Bool {
        packets.send(packet, kind)
    }

    /// Paquet qui ne se rattrape pas : réessayé jusqu'à succès ou 200 ms (voir PacketQueue).
    private func sendReliably(_ kind: String, _ make: @escaping () -> OpaquePointer?) {
        packets.sendReliably(kind, make)
    }

    func mouseMove(dx: Int16, dy: Int16) {
        send(kynput_mouse_move_to_packet(KYNPUT_INPUT_TARGET_HOST, dx, dy), "déplacement relatif")
    }

    func key(scancode: UInt16, pressed: Bool) {
        sendReliably("touche") { kynput_keyboard_key_to_packet(KYNPUT_INPUT_TARGET_HOST, scancode, pressed) }
    }

    func mouse(x: Int16, y: Int16) {
        send(kynput_mouse_position_to_packet(KYNPUT_INPUT_TARGET_HOST, displayID, x, y), "position")
    }

    func mouseButton(_ button: MouseButton, pressed: Bool) {
        sendReliably("clic") { kynput_mouse_button_to_packet(KYNPUT_INPUT_TARGET_HOST, button.sdk, pressed) }
    }

    func wheel(dx: Float, dy: Float) {
        send(kynput_mouse_wheel_to_packet(KYNPUT_INPUT_TARGET_HOST, dx, dy), "molette")
    }

    /// Appelé quand les entrées rejoignent l'ordinateur (début de session,
    /// reconnexion) : l'Apple TV y réannonce ses manettes.
    var onConnected: (() -> Void)?

    func networkConnected() {
        KyberLog.app.notice("entrées reliées à l'ordinateur")
        onConnected?()
    }

    /// L'ordinateur relâche tout ce qu'il tient de nous : touches, boutons de
    /// souris, boutons et axes des manettes. Même file que les autres paquets,
    /// donc réessayé s'il est refusé.
    func flushKeys() {
        sendReliably("relâchement général") { kynput_control_event_to_packet(KYNPUT_INPUT_TARGET_HOST, KYNPUT_CONTROL_EVENT_FLUSH_KEYS) }
    }

    // MARK: Manettes produites par l'app (Apple TV)

    func gamepadPlug(id: UInt32, vendor: UInt16?, product: UInt16?) {
        sendReliably("manette branchée") {
            guard var v = vendor, var p = product else { return kynput_gamepad_plug_to_packet(KYNPUT_INPUT_TARGET_HOST, id, nil, nil) }
            return kynput_gamepad_plug_to_packet(KYNPUT_INPUT_TARGET_HOST, id, &v, &p)
        }
    }

    func gamepadUnplug(id: UInt32) {
        lock.lock()
        lastAxis[id] = nil
        pendingAxes[id] = nil
        lock.unlock()
        sendReliably("manette débranchée") { kynput_gamepad_unplug_to_packet(KYNPUT_INPUT_TARGET_HOST, id) }
    }

    func gamepadButton(id: UInt32, _ button: PadButton, pressed: Bool) {
        sendReliably("manette") {
            var b = kynput_GamepadEventButton(id: id, type_: button.sdk, pressed: pressed)
            return kynput_gamepad_button_to_packet(KYNPUT_INPUT_TARGET_HOST, &b)
        }
    }

    // Axes : un axe ne part que s'il a bougé (au 1/512 de la course), et les
    // axes d'une tranche de 4 ms partent ensemble : GameController signale X et
    // Y d'un stick même si un seul a changé, et une DualSense en Bluetooth
    // produisait jusqu'à ~1 500 paquets par seconde, de quoi remplir la file
    // d'envoi de kynput (64 paquets, jetés sans retour) et perdre des
    // relâchements. Un axe en mouvement se rattrape au suivant ; son retour au
    // repos, non : il part par la file de secours. Une valeur refusée (file
    // pleine, ou un retour au repos qui attend encore devant elle) reste en
    // attente de la tranche suivante. Les touches partent tout de suite, elles.

    /// Dernière valeur acceptée et valeurs en attente, par manette et axe. Sous le verrou.
    private var lastAxis: [UInt32: [PadAxis: Int16]] = [:]
    private var pendingAxes: [UInt32: [PadAxis: Int16]] = [:]
    private var axisFlushScheduled = false
    private static let axisSlice: DispatchTimeInterval = .milliseconds(4)

    /// Changement au 1/512 de la course ; un retour exact au repos passe toujours.
    static func axisMoved(from last: Int16?, to value: Int16) -> Bool {
        guard let last else { return true }
        if value == 0 { return last != 0 }
        return last >> 7 != value >> 7
    }

    /// Valeur d'un axe produit par l'app : filtrée, regroupée, envoyée (voir ci-dessus).
    func gamepadAxis(id: UInt32, _ axis: PadAxis, value: Int16) {
        lock.lock()
        defer { lock.unlock() }
        guard Self.axisMoved(from: lastAxis[id]?[axis], to: value) else { return }
        lastAxis[id, default: [:]][axis] = value
        pendingAxes[id, default: [:]][axis] = value
        scheduleAxisFlush()
    }

    /// Les six axes d'une manette au repos, tout de suite et sûrement (l'app
    /// prend la manette, arrière-plan, fin de session) ; ce qui attendait est oublié.
    func gamepadAxesRest(id: UInt32) {
        lock.lock()
        pendingAxes[id] = nil
        for axis in PadCombo.axes { lastAxis[id, default: [:]][axis] = 0 }
        lock.unlock()
        for axis in PadCombo.axes { sendAxis(id: id, axis, value: 0) }
    }

    // Sous le verrou.
    private func scheduleAxisFlush() {
        guard !axisFlushScheduled else { return }
        axisFlushScheduled = true
        queue.asyncAfter(deadline: .now() + Self.axisSlice) { [weak self] in self?.flushAxes() }
    }

    private func flushAxes() {
        lock.lock()
        defer { lock.unlock() }
        axisFlushScheduled = false
        var again = false
        for (id, axes) in pendingAxes {
            for (axis, value) in axes where sendAxis(id: id, axis, value: value) {
                pendingAxes[id]?[axis] = nil
            }
            if pendingAxes[id]?.isEmpty == true { pendingAxes[id] = nil } else { again = true }
        }
        if again { scheduleAxisFlush() }
    }

    /// `false` : la valeur n'est pas partie (à représenter).
    @discardableResult
    private func sendAxis(id: UInt32, _ axis: PadAxis, value: Int16) -> Bool {
        if value == 0 {
            sendReliably("axe de manette") {
                var a = kynput_GamepadEventAxis(id: id, type_: axis.sdk, value: 0)
                return kynput_gamepad_axis_to_packet(KYNPUT_INPUT_TARGET_HOST, &a)
            }
            return true
        }
        var a = kynput_GamepadEventAxis(id: id, type_: axis.sdk, value: value)
        return send(kynput_gamepad_axis_to_packet(KYNPUT_INPUT_TARGET_HOST, &a), "axe de manette")
    }

    /// Texte tapé (clavier de l'écran, de l'iPhone) : un caractère à la fois.
    func type(_ text: String) {
        for scalar in text.unicodeScalars {
            sendReliably("texte") { kynput_unicode_to_packet(KYNPUT_INPUT_TARGET_HOST, scalar.value) }
        }
    }

    enum MouseButton {
        case left, right, middle, side, extra

        var sdk: kynput_MouseButtonType {
            switch self {
            case .left: return KYNPUT_MOUSE_BUTTON_TYPE_LEFT
            case .right: return KYNPUT_MOUSE_BUTTON_TYPE_RIGHT
            case .middle: return KYNPUT_MOUSE_BUTTON_TYPE_MIDDLE
            case .side: return KYNPUT_MOUSE_BUTTON_TYPE_SIDE
            case .extra: return KYNPUT_MOUSE_BUTTON_TYPE_EXTRA
            }
        }
    }

    /// Forme ARGB (32 bits, non prémultipliée) → CGImage (prémultiplié, sRGB).
    private static func decode(_ c: OpaquePointer, lastShape: inout Int?) -> RemoteCursor {
        let visible = kynput_cursor_visible(c)
        let width = Int(kynput_cursor_width(c))
        let height = Int(kynput_cursor_height(c))
        let hot = CGPoint(x: Int(kynput_cursor_xhot(c)), y: Int(kynput_cursor_yhot(c)))
        let position = CGPoint(x: Int(kynput_cursor_x(c)), y: Int(kynput_cursor_y(c)))
        var count: UInt32 = 0
        guard kynput_cursor_format(c) == KYNPUT_CURSOR_FORMAT_RAW,
              let raw = kynput_cursor_raw_data(c, &count),
              width > 0, height > 0, Int(count) >= width * height
        else { return RemoteCursor(visible: visible, image: nil, hotSpot: hot, position: position) }
        // Même forme que la dernière fois : seule la position change.
        var hasher = Hasher()
        hasher.combine(width)
        hasher.combine(height)
        hasher.combine(bytes: UnsafeRawBufferPointer(start: raw, count: width * height * 4))
        let shape = hasher.finalize()
        if shape == lastShape { return RemoteCursor(visible: visible, image: nil, hotSpot: hot, position: position) }
        lastShape = shape

        var pixels = [UInt32](repeating: 0, count: width * height)
        var opaque = false
        for i in 0..<(width * height) {
            let p = raw[i]
            let a = (p >> 24) & 0xFF
            if a > 0 { opaque = true }
            let r = ((p >> 16) & 0xFF) * a / 255
            let g = ((p >> 8) & 0xFF) * a / 255
            let b = (p & 0xFF) * a / 255
            pixels[i] = (a << 24) | (r << 16) | (g << 8) | b
        }
        // Forme entièrement transparente (lecteurs vidéo, certains jeux) :
        // le Mac garderait un pointeur invisible ; on garde la forme précédente.
        guard opaque else { return RemoteCursor(visible: visible, image: nil, hotSpot: hot, position: position) }
        let image: CGImage? = pixels.withUnsafeMutableBytes { buffer in
            guard let ctx = CGContext(
                data: buffer.baseAddress, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
            ) else { return nil }
            return ctx.makeImage()
        }
        return RemoteCursor(visible: visible, image: image, hotSpot: hot, position: position)
    }
}
