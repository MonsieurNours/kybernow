// Les entrées du Mac sur le cœur Rust (jalon 2 de docs/coeur-apple.md) :
// chaîne kynput, file de secours, clavier, combinaison et prise des
// manettes SDL, tout relâcher, pointeur de l'ordinateur, presse-papiers
// sont dans `kybernow_core::input`, par l'API C de apps/apple. Ce fichier
// garde les noms qu'utilisaient StreamView et SessionEngine (InputPipeline,
// PointerLayer, Keymap, MouseModeRules) ; Swift ne fait plus que capter les
// événements d'AppKit, dessiner le pointeur et traduire kVK → HID
// (KeymapMac.swift).
//
// Mac seulement : l'Apple TV compile encore les versions Swift (Keymap,
// PadCombo, PointerLayer, ReleaseAll, ClipboardBridge, PacketQueue,
// InputPipeline, GamepadGate) jusqu'au jalon 4.

import AppKit
import CoreGraphics
import Foundation
@_implementationOnly import CKyberNow

/// Bureau : souris absolue, pointeur de l'appareil toujours là. Jeu : souris
/// relative (déplacements bruts). Auto : jeu quand un jeu est en plein écran
/// sur l'ordinateur et cache son pointeur.
public enum MouseMode: Sendable { case auto, desktop, game }

/// Touche de navigation pendant que la manette pilote l'app.
public enum PadKey: Sendable {
    case left, right, up, down, accept, back
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
        /// Position annoncée par l'ordinateur (pixels de son écran).
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

    /// L'écran de l'ordinateur a changé de taille en cours de session.
    func setHostSize(_ size: CGSize) {
        kn_pointer_set_host_size(core, size.width, size.height)
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

    /// Ce que le cœur rappelle (fils du cœur) : le fil principal pour l'interface.
    private final class Callbacks {
        weak var gamepads: Gamepads?
        let pasteboard = NSPasteboard.general
    }

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
            gamepads_from_app: false
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
            on_connected: nil,
            on_rumble: nil,
            pasteboard_change_count: { ud in
                guard let ud else { return 0 }
                let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
                return UInt64(max(0, box.pasteboard.changeCount))
            },
            pasteboard_text: { ud in
                guard let ud else { return nil }
                let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
                guard let text = box.pasteboard.string(forType: .string) else { return nil }
                return strdup(text)
            },
            pasteboard_set_text: { ud, text in
                guard let ud, let text else { return }
                let box = Unmanaged<Callbacks>.fromOpaque(ud).takeUnretainedValue()
                box.pasteboard.clearContents()
                box.pasteboard.setString(String(cString: text), forType: .string)
            }
        )
        core = kn_input_new(options, pointer.core, callbacks)
        gamepad.owner = self
    }

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
    /// Les entrées rejoignent l'ordinateur (début de session, reconnexion).
    func networkConnected() { kn_input_network_connected(core) }
    /// Fin de l'image : plus rien ne part, l'ordinateur relâche tout.
    func stop() { kn_input_stop(core) }
    /// Arrête les gestionnaires de kynput (manettes SDL).
    func shutdown() { kn_input_shutdown(core) }
}
