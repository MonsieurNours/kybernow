// Les entrées des apps Apple sur le cœur Rust (docs/coeur-apple.md) : chaîne
// kynput, file de secours, clavier, combinaison et prise des manettes, tout
// relâcher, pointeur de l'ordinateur, presse-papiers sont dans
// `kybernow_core::input`, par l'API C de apps/apple. Ce fichier en donne la
// façade Swift aux vues et aux ponts de manettes (InputPipeline,
// PointerLayer, PadCombo, Keymap, MouseModeRules) ; chaque plateforme ne fait
// que capter ses événements (AppKit sur Mac ; GameController et UIKit sur
// Apple TV, iPhone et iPad), dessiner le pointeur et, sur Mac, traduire
// kVK → HID (KeymapMac.swift).

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif
import CoreGraphics
import Foundation
@_implementationOnly import CKyber
@_implementationOnly import CKyberNow

/// File série unique des entrées de l'appareil hors du fil principal
/// (GameController sur Apple TV et iOS : manettes, télécommande, clavier et
/// souris Bluetooth).
public enum InputQueue {
    public static let device = DispatchQueue(label: "fr.leslignesclaires.kybernow.manettes", qos: .userInteractive)
}

/// Bureau : souris absolue, pointeur de l'appareil toujours là. Jeu : souris
/// relative (déplacements bruts). Auto : jeu quand un jeu est en plein écran
/// sur l'ordinateur et cache son pointeur.
public enum MouseMode: Sendable { case auto, desktop, game }

/// Touche de navigation pendant que la manette pilote l'app.
public enum PadKey: Sendable {
    case left, right, up, down, accept, back
}

/// Touche de manette, dans le vocabulaire de kynput.
public enum PadButton: Sendable, CaseIterable, Hashable {
    case a, b, x, y, back, guide, start, leftStick, rightStick, leftShoulder, rightShoulder
    case dpadUp, dpadDown, dpadLeft, dpadRight

    var core: UInt32 {
        switch self {
        case .a: return KYNPUT_GAMEPAD_BUTTON_A.rawValue
        case .b: return KYNPUT_GAMEPAD_BUTTON_B.rawValue
        case .x: return KYNPUT_GAMEPAD_BUTTON_X.rawValue
        case .y: return KYNPUT_GAMEPAD_BUTTON_Y.rawValue
        case .back: return KYNPUT_GAMEPAD_BUTTON_BACK.rawValue
        case .guide: return KYNPUT_GAMEPAD_BUTTON_GUIDE.rawValue
        case .start: return KYNPUT_GAMEPAD_BUTTON_START.rawValue
        case .leftStick: return KYNPUT_GAMEPAD_BUTTON_LEFT_STICK.rawValue
        case .rightStick: return KYNPUT_GAMEPAD_BUTTON_RIGHT_STICK.rawValue
        case .leftShoulder: return KYNPUT_GAMEPAD_BUTTON_LEFT_SHOULDER.rawValue
        case .rightShoulder: return KYNPUT_GAMEPAD_BUTTON_RIGHT_SHOULDER.rawValue
        case .dpadUp: return KYNPUT_GAMEPAD_BUTTON_D_PAD_UP.rawValue
        case .dpadDown: return KYNPUT_GAMEPAD_BUTTON_D_PAD_DOWN.rawValue
        case .dpadLeft: return KYNPUT_GAMEPAD_BUTTON_D_PAD_LEFT.rawValue
        case .dpadRight: return KYNPUT_GAMEPAD_BUTTON_D_PAD_RIGHT.rawValue
        }
    }

    init?(core: UInt32) {
        guard let button = Self.allCases.first(where: { $0.core == core }) else { return nil }
        self = button
    }
}

/// Axe de manette, dans le vocabulaire de kynput.
public enum PadAxis: Sendable, CaseIterable, Hashable {
    case leftX, leftY, rightX, rightY, leftTrigger, rightTrigger

    var core: UInt32 {
        switch self {
        case .leftX: return KYNPUT_GAMEPAD_AXIS_LEFT_X.rawValue
        case .leftY: return KYNPUT_GAMEPAD_AXIS_LEFT_Y.rawValue
        case .rightX: return KYNPUT_GAMEPAD_AXIS_RIGHT_X.rawValue
        case .rightY: return KYNPUT_GAMEPAD_AXIS_RIGHT_Y.rawValue
        case .leftTrigger: return KYNPUT_GAMEPAD_AXIS_TRIGGER_LEFT.rawValue
        case .rightTrigger: return KYNPUT_GAMEPAD_AXIS_TRIGGER_RIGHT.rawValue
        }
    }
}

/// Vibration demandée par le jeu pour une manette (moteurs de 0 à 65535,
/// durée en millisecondes, -1 : jusqu'à nouvel ordre).
public struct GamepadRumble: Sendable {
    public let id: UInt32
    public let largeMotor: UInt16
    public let smallMotor: UInt16
    public let durationMs: Int
}

/// Combinaison L1 + R1 + Start + Select (250 ms tenues) et touches tenues de
/// chaque manette, tenues par le cœur : pour un pont de manettes qui la suit
/// lui-même avant d'envoyer (GameController sur Apple TV et iOS).
final class PadCombo: @unchecked Sendable {
    static let buttons: Set<PadButton> = [.leftShoulder, .rightShoulder, .start, .back]
    /// Les six axes à remettre au repos à la capture.
    static let axes: [PadAxis] = PadAxis.allCases

    /// Combinaison formée pour cette manette, sur la file donnée.
    var onFormed: ((UInt32) -> Void)?
    /// Réglage « la combinaison ouvre le menu » ; `false` : rien n'est armé.
    var enabled = true { didSet { kn_combo_set_enabled(core, enabled) } }

    private let queue: DispatchQueue
    private var core: OpaquePointer!

    init(on queue: DispatchQueue) {
        self.queue = queue
        let callbacks = KnComboCallbacks(userdata: Unmanaged.passUnretained(self).toOpaque(), on_formed: { ud, pad in
            guard let ud else { return }
            let me = Unmanaged<PadCombo>.fromOpaque(ud).takeUnretainedValue()
            me.queue.async { me.onFormed?(pad) }
        })
        core = kn_combo_new(callbacks)
    }

    deinit { kn_combo_free(core) }

    func pressed(_ id: UInt32) -> Set<PadButton> {
        var out = [UInt32](repeating: 0, count: 16)
        let n = out.withUnsafeMutableBufferPointer { kn_combo_pressed(core, id, $0.baseAddress, UInt(16)) }
        return Set(out.prefix(Int(n)).compactMap { PadButton(core: $0) })
    }

    func formed(_ id: UInt32) -> Bool { kn_combo_formed(core, id) }
    func forget(_ id: UInt32) { kn_combo_forget(core, id) }

    /// Les touches tenues d'une manette, d'après son état réel.
    func set(pressed buttons: Set<PadButton>, for id: UInt32) {
        let list = buttons.map(\.core)
        list.withUnsafeBufferPointer { kn_combo_set_pressed(core, id, $0.baseAddress, UInt($0.count)) }
    }

    func button(_ id: UInt32, _ button: PadButton, pressed down: Bool) {
        kn_combo_button(core, id, button.core, down)
    }

    static func navigationKey(_ button: PadButton) -> PadKey? {
        switch button {
        case .dpadLeft: return .left
        case .dpadRight: return .right
        case .dpadUp: return .up
        case .dpadDown: return .down
        case .a: return .accept
        case .b: return .back
        default: return nil
        }
    }
}

/// Règles Auto / Bureau / Jeu (le cœur), avec la veille du plein écran.
struct MouseModeRules {
    let needsHiddenPointer: Bool

    /// Un pointeur remontré un instant ne fait pas quitter le mode jeu : on revérifie après ce délai.
    static let recheckDelay = TimeInterval(KN_MOUSE_RECHECK_DELAY_MS) / 1000

    func wantsRelative(mode: MouseMode, gameFullscreen: Bool, remoteHidden: Bool, visibleFor: TimeInterval, relativeActive: Bool) -> Bool {
        let m: UInt32
        switch mode {
        case .auto: m = UInt32(KN_MOUSE_AUTO)
        case .desktop: m = UInt32(KN_MOUSE_DESKTOP)
        case .game: m = UInt32(KN_MOUSE_GAME)
        }
        return kn_mouse_wants_relative(needsHiddenPointer, m, gameFullscreen, remoteHidden, visibleFor * 1000, relativeActive)
    }
}

/// Clavier : usages HID → scancodes, « ⌘ envoyée comme Ctrl », échange des
/// claviers Apple ISO, verrouillages (le cœur). La table kVK → HID et la
/// disposition du clavier sont dans KeymapMac.swift.
public enum Keymap {
    static func isLock(_ usage: UInt16) -> Bool { kn_keymap_is_lock(usage) }
    static func isModifier(_ usage: UInt16) -> Bool { (0xE0...0xE7).contains(usage) }

    /// Apple TV et iOS : le réglage « Clavier Apple ISO » s'applique aux
    /// claviers dont le fabricant est Apple, ou inconnu.
    static func appleKeyboard(vendorName: String?) -> Bool {
        kn_keymap_apple_keyboard(vendorName)
    }

    /// Scancode d'un usage HID tel quel (sans échange ni « ⌘ envoyée comme Ctrl »).
    static func scancode(usage: UInt16) -> UInt16? {
        let code = kn_keymap_scancode(usage, false, false)
        return code >= 0 ? UInt16(code) : nil
    }

    /// Scancode à envoyer pour une touche donnée en usage HID ; `iso` n'est
    /// évalué que pour les deux touches concernées (il interroge le système).
    static func scancode(hid: UInt16, commandAsControl: Bool = false, iso: @autoclosure () -> Bool) -> UInt16? {
        let isISO = kn_keymap_needs_iso(hid) ? iso() : false
        let code = kn_keymap_scancode(hid, commandAsControl, isISO)
        return code >= 0 ? UInt16(code) : nil
    }
}

// MARK: - Pointeur

/// Pointeur de l'ordinateur, tenu par le cœur ; une couche par vue de
/// session, qui survit aux relances de l'image.
final class PointerLayer: @unchecked Sendable {
    /// Forme du pointeur, en pixels de l'écran de l'ordinateur.
    struct Shape {
        let image: CGImage
        let hotSpot: CGPoint
        var pixels: CGSize { CGSize(width: image.width, height: image.height) }
    }

    struct State {
        /// Position du pointeur, en pixels de l'écran de l'ordinateur : celle
        /// tenue par l'appareil (Apple TV), sinon celle annoncée par l'ordinateur.
        var remotePosition: CGPoint?
        /// L'ordinateur cache son pointeur.
        var remoteHidden = false
        /// Depuis combien de temps l'ordinateur montre son pointeur (0 : caché).
        var visibleFor: TimeInterval = 0
        /// Dernière forme reçue (un masquage ou une forme vide la garde).
        var shape: Shape?
    }

    /// Ce qui a changé ; dans tous les cas, à redessiner.
    struct Change {
        var visibility = false
        var shape = false
    }

    /// File où `onChanged` est appelé (le fil principal sur Mac).
    let queue: DispatchQueue
    /// L'état a changé : la vue redessine.
    var onChanged: ((State, Change) -> Void)?

    fileprivate var core: OpaquePointer!
    private let lock = NSLock()
    private var shape: Shape?

    init(queue: DispatchQueue, holdsPosition: Bool) {
        self.queue = queue
        let callbacks = KnPointerCallbacks(
            userdata: Unmanaged.passUnretained(self).toOpaque(),
            on_changed: { userdata, state, visibility, shape in
                guard let userdata, let state else { return }
                let me = Unmanaged<PointerLayer>.fromOpaque(userdata).takeUnretainedValue()
                me.changed(state.pointee, Change(visibility: visibility, shape: shape))
            }
        )
        core = kn_pointer_new(holdsPosition, callbacks)
    }

    deinit {
        kn_pointer_free(core)
    }

    /// Fil de kynput : la forme devient une image (si elle a changé), la vue
    /// est prévenue sur sa file.
    private func changed(_ s: KnPointerState, _ change: Change) {
        if change.shape, s.has_shape, let pixels = s.pixels, let image = Self.image(pixels, width: Int(s.width), height: Int(s.height)) {
            lock.lock()
            shape = Shape(image: image, hotSpot: CGPoint(x: Int(s.hot_x), y: Int(s.hot_y)))
            lock.unlock()
        }
        let state = state(from: s)
        queue.async { [weak self] in self?.onChanged?(state, change) }
    }

    /// État en ce moment (de n'importe quel fil).
    var state: State {
        var s = KnPointerState()
        kn_pointer_state(core, &s)
        return state(from: s)
    }

    private func state(from s: KnPointerState) -> State {
        lock.lock()
        let shape = self.shape
        lock.unlock()
        return State(
            remotePosition: s.has_position ? CGPoint(x: s.x, y: s.y) : nil,
            remoteHidden: s.remote_hidden,
            visibleFor: s.visible_for_ms / 1000,
            shape: shape
        )
    }

    /// Multi-écrans : le pointeur de l'ordinateur pour la souris Auto sur
    /// toutes ces couches (règle du cœur : caché seulement s'il l'est sur
    /// tous les écrans où il est déjà passé, visible depuis le plus long).
    static func summary(_ layers: [PointerLayer]) -> (hidden: Bool, visibleFor: TimeInterval) {
        let cores: [OpaquePointer?] = layers.map(\.core)
        var hidden = false, ms = 0.0
        cores.withUnsafeBufferPointer { kn_pointer_summary($0.baseAddress, UInt($0.count), &hidden, &ms) }
        return (hidden, ms / 1000)
    }

    /// L'écran de l'ordinateur a changé de taille en cours de session.
    func setHostSize(_ size: CGSize) {
        kn_pointer_set_host_size(core, size.width, size.height)
    }

    /// Position tenue par l'appareil déplacée (Apple TV : télécommande,
    /// manette, souris en positions absolues) ; l'ordinateur la reçoit.
    func move(dx: Double, dy: Double) {
        kn_pointer_move_by(core, dx, dy)
    }

    /// Position tenue renvoyée (entrées reliées de nouveau).
    func resendHeld() {
        kn_pointer_resend_held(core)
    }

    /// Fin de session.
    func reset() {
        kn_pointer_reset(core)
        lock.lock()
        shape = nil
        lock.unlock()
    }

    /// ARGB prémultiplié sur 32 bits (BGRA en mémoire) → image.
    private static func image(_ pixels: UnsafePointer<UInt32>, width: Int, height: Int) -> CGImage? {
        guard width > 0, height > 0 else { return nil }
        let data = Data(bytes: pixels, count: width * height * 4)
        guard let provider = CGDataProvider(data: data as CFData) else { return nil }
        let info = CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        return CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
                       space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: info, provider: provider, decode: nil,
                       shouldInterpolate: true, intent: .defaultIntent)
    }

    // MARK: - Dessin

    /// Facteur d'échelle d'une forme pour la dessiner dans une case de `box`
    /// points (Windows dessine ses pointeurs dans une case de 32 pixels à
    /// 100 %, 48 à 150 %…). Mac : 32 points, jamais agrandi.
    static func scale(pixels: CGSize, box: CGFloat, maxFactor: CGFloat) -> CGFloat {
        CGFloat(kn_pointer_scale(UInt32(pixels.width), UInt32(pixels.height), Double(box), Double(maxFactor)))
    }

    /// Cadre du pointeur dessiné : `local` est la pointe dans la vue, y vers
    /// le bas ; `flippedHeight` : hauteur de la vue si son y va vers le haut.
    static func frame(shape: Shape, local: CGPoint, factor: CGFloat, flippedHeight: CGFloat? = nil) -> CGRect {
        let size = CGSize(width: shape.pixels.width * factor, height: shape.pixels.height * factor)
        let x = local.x - shape.hotSpot.x * factor
        let top = local.y - shape.hotSpot.y * factor
        if let height = flippedHeight {
            return CGRect(x: x, y: height - top - size.height, width: size.width, height: size.height)
        }
        return CGRect(x: x, y: top, width: size.width, height: size.height)
    }
}

// MARK: - Chaîne d'entrées

/// La chaîne d'entrées d'une image : ce que la vue capte part par le cœur.
final class InputPipeline: @unchecked Sendable {
    enum MouseButton {
        case left, right, middle, side, extra

        var core: UInt32 {
            switch self {
            case .left: return UInt32(KN_MOUSE_LEFT)
            case .right: return UInt32(KN_MOUSE_RIGHT)
            case .middle: return UInt32(KN_MOUSE_MIDDLE)
            case .side: return UInt32(KN_MOUSE_SIDE)
            case .extra: return UInt32(KN_MOUSE_EXTRA)
            }
        }
    }

    /// Manettes lues par kynput via SDL : combinaison L1 + R1 + Start + Select
    /// (250 ms tenues) et prise de la manette par la barre, dans le cœur.
    final class Gamepads: @unchecked Sendable {
        /// Combinaison formée (fil principal).
        var onCombo: (() -> Void)?
        /// Touche de navigation pendant que l'app a la manette (fil principal).
        var onNavigate: ((PadKey) -> Void)?
        fileprivate weak var owner: InputPipeline?

        /// L'app prend la manette (barre ouverte) ou la rend (barre fermée).
        func setCaptured(_ captured: Bool) {
            guard let core = owner?.core else { return }
            kn_input_set_gamepad_captured(core, captured)
        }
    }

    let displayID: UInt32
    let hostSize: CGSize
    let gamepad = Gamepads()
    fileprivate var core: OpaquePointer?
    /// Rappels du cœur : gardé vivant un peu après la fin de la chaîne (le
    /// fil des minuteries peut encore appeler pendant qu'elle se défait).
    private let box: Callbacks

    /// Vibrations des manettes produites par l'app (Apple TV, iPhone, iPad : kynput ne les joue pas lui-même) ; fil de kynput.
    var onRumble: ((GamepadRumble) -> Void)? {
        get { box.lock.withLock { box.rumble } }
        set { box.lock.withLock { box.rumble = newValue } }
    }

    /// Les entrées rejoignent l'ordinateur (début de session, reconnexion).
    var onConnected: (() -> Void)? {
        get { box.lock.withLock { box.connected } }
        set { box.lock.withLock { box.connected = newValue } }
    }

    /// Ce que le cœur rappelle (fils du cœur) : le fil principal pour l'interface.
    private final class Callbacks {
        weak var gamepads: Gamepads?
        let lock = NSLock()
        var rumble: ((GamepadRumble) -> Void)?
        var connected: (() -> Void)?
        #if os(macOS)
        let pasteboard = NSPasteboard.general
        /// Plusieurs ordinateurs : presse-papiers en pause (session inactive),
        /// et le compteur de changements figé à la mise en pause.
        var clipboardPaused = false
        var frozenChangeCount = 0
        #endif
    }

    /// Manettes produites par l'app (GameController) ; sur Mac, kynput les lit via SDL.
    #if os(macOS)
    private static let gamepadsFromApp = false
    #else
    private static let gamepadsFromApp = true
    #endif

    /// `pointer` : la couche de pointeur de la vue de session.
    init(displayID: UInt32, hostSize: CGSize, pointer: PointerLayer, clipboard: Bool) {
        self.displayID = displayID
        self.hostSize = hostSize
        box = Callbacks()
        box.gamepads = gamepad
        let options = KnInputOptions(
            display_id: displayID,
            host_width: UInt32(max(0, hostSize.width)),
            host_height: UInt32(max(0, hostSize.height)),
            clipboard: clipboard,
            gamepads_from_app: Self.gamepadsFromApp
        )
        let ud = Unmanaged.passRetained(box).toOpaque()
        let callbacks = KnInputCallbacks(
            userdata: ud,
            on_combo: { ud, _ in
                guard let ud else { return }
                let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
                KyberLog.app.notice("combinaison L1 + R1 + Start + Select")
                DispatchQueue.main.async { box.gamepads?.onCombo?() }
            },
            on_navigate: { ud, key in
                guard let ud else { return }
                let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
                let k: PadKey
                switch Int32(key) {
                case KN_PAD_KEY_LEFT: k = .left
                case KN_PAD_KEY_RIGHT: k = .right
                case KN_PAD_KEY_UP: k = .up
                case KN_PAD_KEY_DOWN: k = .down
                case KN_PAD_KEY_ACCEPT: k = .accept
                default: k = .back
                }
                DispatchQueue.main.async { box.gamepads?.onNavigate?(k) }
            },
            on_connected: { ud in
                guard let ud else { return }
                let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
                box.lock.withLock { box.connected }?()
            },
            on_rumble: { ud, pad, large, small, duration in
                guard let ud else { return }
                let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
                box.lock.withLock { box.rumble }?(GamepadRumble(id: pad, largeMotor: large, smallMotor: small, durationMs: Int(duration)))
            },
            pasteboard_change_count: Self.pasteboardChangeCount,
            pasteboard_text: Self.pasteboardText,
            pasteboard_set_text: Self.pasteboardSetText,
            pasteboard_has_text: Self.pasteboardHasText,
            pasteboard_reading_asks_user: Self.pasteboardReadingAsksUser
        )
        core = kn_input_new(options, pointer.core, callbacks)
        gamepad.owner = self
        // Apple TV, iPhone, iPad : la combinaison est suivie par le pont de manettes (PadCombo), avant l'envoi.
        if Self.gamepadsFromApp { kn_input_set_combo_enabled(core, false) }
    }

    #if os(macOS)
    // En pause (`setClipboardActive(false)`) : le compteur reste figé, aucun
    // texte n'est lu ni écrit sur ce Mac.
    private static let pasteboardChangeCount: (@convention(c) (UnsafeMutableRawPointer?) -> UInt64)? = { ud in
        guard let ud else { return 0 }
        let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
        let frozen = box.lock.withLock { box.clipboardPaused ? box.frozenChangeCount : nil }
        return UInt64(max(0, frozen ?? box.pasteboard.changeCount))
    }
    private static let pasteboardText: (@convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?)? = { ud in
        guard let ud else { return nil }
        let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
        guard !box.lock.withLock({ box.clipboardPaused }), let text = box.pasteboard.string(forType: .string) else { return nil }
        return strdup(text)
    }
    private static let pasteboardSetText: (@convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void)? = { ud, text in
        guard let ud, let text else { return }
        let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
        guard !box.lock.withLock({ box.clipboardPaused }) else { return }
        box.pasteboard.clearContents()
        box.pasteboard.setString(String(cString: text), forType: .string)
    }
    private static let pasteboardHasText: (@convention(c) (UnsafeMutableRawPointer?) -> Bool)? = nil
    private static let pasteboardReadingAsksUser = false
    #elseif os(iOS)
    // UIPasteboard : le compteur se lit librement ; lire le texte fait
    // demander à iOS l'accord de coller (une fois par changement, à la copie
    // d'un texte dans une autre app pendant la session).
    private static let pasteboardChangeCount: (@convention(c) (UnsafeMutableRawPointer?) -> UInt64)? = { _ in
        UInt64(max(0, UIPasteboard.general.changeCount))
    }
    private static let pasteboardText: (@convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?)? = { _ in
        guard UIPasteboard.general.hasStrings, let text = UIPasteboard.general.string else { return nil }
        return strdup(text)
    }
    private static let pasteboardSetText: (@convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void)? = { _, text in
        guard let text else { return }
        UIPasteboard.general.string = String(cString: text)
    }
    // `hasStrings` ne demande rien ; le texte n'est lu que quand l'ordinateur
    // le demande (« Autoriser à coller ? » ferait perdre le premier plan et
    // figerait l'image).
    private static let pasteboardHasText: (@convention(c) (UnsafeMutableRawPointer?) -> Bool)? = { _ in
        UIPasteboard.general.hasStrings
    }
    private static let pasteboardReadingAsksUser = true
    #else
    // Pas de presse-papiers sur l'Apple TV.
    private static let pasteboardChangeCount: (@convention(c) (UnsafeMutableRawPointer?) -> UInt64)? = nil
    private static let pasteboardText: (@convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?)? = nil
    private static let pasteboardSetText: (@convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?) -> Void)? = nil
    private static let pasteboardHasText: (@convention(c) (UnsafeMutableRawPointer?) -> Bool)? = nil
    private static let pasteboardReadingAsksUser = false
    #endif

    deinit {
        kn_input_free(core)
        let ud = Unmanaged.passUnretained(box).toOpaque()
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            Unmanaged<Callbacks>.fromOpaque(ud).release()
        }
    }

    /// La chaîne kynput avec une référence prise, pour la session.
    func kynputRef() -> UnsafeMutableRawPointer? {
        kn_input_kynput(core)
    }

    /// Souris relative (jeux) : seuls les déplacements partent.
    func setRelative(_ on: Bool) { kn_input_set_relative(core, on) }
    var isRelative: Bool { kn_input_is_relative(core) }

    func mouse(x: Int16, y: Int16) { kn_input_mouse(core, x, y) }

    /// Multi-écrans : un écran de la session vu des entrées (`on_screens` du cœur).
    struct Screen {
        /// Index de l'écran de l'appareil.
        let index: Int
        /// Identifiant d'écran de Kyber et sa taille en pixels.
        let displayID: UInt32
        let hostSize: CGSize
        /// Couche de pointeur de la vue de cet écran (nil : celle de la chaîne).
        let pointer: PointerLayer?
    }

    /// Couches de pointeur données au cœur : gardées tant que la chaîne vit
    /// (le cœur rappelle chacune depuis le fil de kynput).
    private var screenPointers: [PointerLayer] = []
    private let screensLock = NSLock()

    /// Multi-écrans : les écrans de la session ; le pointeur de l'ordinateur
    /// va à la couche de l'écran qu'il annonce, les positions de
    /// `mouse(on:x:y:)` à l'écran de Kyber de leur écran.
    func setScreens(_ screens: [Screen]) {
        let pointers = screens.compactMap(\.pointer)
        screensLock.lock()
        screenPointers = pointers
        screensLock.unlock()
        var list = screens.map { s in
            KnInputScreen(screen: UInt32(s.index), display_id: s.displayID, host_width: UInt32(max(0, s.hostSize.width)),
                          host_height: UInt32(max(0, s.hostSize.height)), pointer: s.pointer?.core)
        }
        list.withUnsafeMutableBufferPointer { kn_input_set_screens(core, $0.baseAddress, UInt($0.count)) }
    }

    /// Multi-écrans : position absolue sur l'écran `index` de l'appareil, en
    /// pixels de son écran de l'ordinateur.
    func mouse(on index: Int, x: Int16, y: Int16) { kn_input_mouse_on(core, UInt32(index), x, y) }
    func mouseMove(dx: Int16, dy: Int16) { kn_input_mouse_move(core, dx, dy) }
    func mouseButton(_ button: MouseButton, pressed: Bool) { kn_input_mouse_button(core, button.core, pressed) }
    func wheel(dx: Float, dy: Float) { kn_input_wheel(core, dx, dy) }
    /// Touche par son scancode, notée pour « tout relâcher ».
    func key(scancode: UInt16, pressed: Bool) { kn_input_key(core, scancode, pressed) }
    /// L'ordinateur relâche tout ce qu'il tient de nous, manettes comprises.
    func flushKeys() { kn_input_flush_keys(core) }
    /// Relâche les touches et clics tenus (la fenêtre perd le clavier), sans les manettes.
    func releaseAll() { kn_input_release_all(core) }

    /// Touche de modification (code kVK) tenue ou relâchée ; `false` : déjà dans cet état.
    @discardableResult
    func modifier(_ code: UInt16, pressed: Bool) -> Bool { kn_input_modifier(core, code, pressed) }
    func modifierHeld(_ code: UInt16) -> Bool { kn_input_modifier_held(core, code) }

    /// Modificateurs tenus chez nous (codes kVK).
    func heldModifiers() -> [UInt16] {
        var out = [UInt16](repeating: 0, count: 16)
        let n = out.withUnsafeMutableBufferPointer { kn_input_held_modifiers(core, $0.baseAddress, UInt(16)) }
        return Array(out.prefix(Int(n)))
    }

    /// Les modificateurs repartent de zéro (service d'entrées neuf).
    func forgetModifiers() { kn_input_forget_modifiers(core) }

    /// Modificateurs réellement tenus (codes kVK) → ceux à renvoyer appuyés.
    func resyncModifiers(held: [UInt16]) -> [UInt16] {
        var out = [UInt16](repeating: 0, count: 16)
        let n = held.withUnsafeBufferPointer { h in
            out.withUnsafeMutableBufferPointer { kn_input_resync_modifiers(core, h.baseAddress, UInt(h.count), $0.baseAddress, UInt(16)) }
        }
        return Array(out.prefix(Int(n)))
    }

    /// Une fois la chaîne branchée au réseau.
    func startClipboard() { kn_input_start_clipboard(core) }

    /// Plusieurs ordinateurs (Mac) : presse-papiers en pause tant que la
    /// session n'est pas active. Rien n'est lu sur ce Mac (le compteur de
    /// changements reste celui de la mise en pause, l'ordinateur qui demande
    /// le texte n'en reçoit pas) et rien n'y est écrit ; revenue active, un
    /// texte copié entre-temps part vers l'ordinateur. Sans effet ailleurs.
    func setClipboardActive(_ active: Bool) {
        #if os(macOS)
        box.lock.withLock {
            if !active, !box.clipboardPaused { box.frozenChangeCount = box.pasteboard.changeCount }
            box.clipboardPaused = !active
        }
        #endif
    }
    /// Les entrées rejoignent l'ordinateur (début de session, reconnexion).
    func networkConnected() { kn_input_network_connected(core) }
    /// Fin de l'image : plus rien ne part, l'ordinateur relâche tout.
    func stop() { kn_input_stop(core) }
    /// Arrête les gestionnaires de kynput (manettes SDL).
    func shutdown() { kn_input_shutdown(core) }

    // MARK: Texte et manettes produites par l'app (Apple TV, iOS)

    /// Texte tapé au clavier de l'écran ou de l'iPhone.
    func type(_ text: String) { kn_input_type_text(core, text) }

    func gamepadPlug(id: UInt32, vendor: UInt16?, product: UInt16?) {
        kn_input_gamepad_plug(core, id, vendor.map(Int32.init) ?? -1, product.map(Int32.init) ?? -1)
    }

    func gamepadUnplug(id: UInt32) { kn_input_gamepad_unplug(core, id) }

    func gamepadButton(id: UInt32, _ button: PadButton, pressed: Bool) {
        kn_input_gamepad_button(core, id, button.core, pressed)
    }

    /// Axe filtré (1/512 de la course) et regroupé par 4 ms ; retour au repos réessayé.
    func gamepadAxis(id: UInt32, _ axis: PadAxis, value: Int16) {
        kn_input_gamepad_axis(core, id, axis.core, value)
    }

    /// Les six axes au repos, tout de suite et sûrement.
    func gamepadAxesRest(id: UInt32) { kn_input_gamepad_axes_rest(core, id) }
}
