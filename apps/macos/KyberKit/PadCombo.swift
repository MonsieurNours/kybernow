// Combinaison manette L1 + R1 + Start + Select, et touches tenues de chaque
// manette : ce que le Mac (GamepadGate, paquets SDL vus par le hook de
// kynput) et l'Apple TV (GamepadBridge, GameController) ont en commun.
//
// La combinaison ouvre la barre (Mac) ou le menu (Apple TV) : la manette
// pilote alors KyberNow et le jeu ne reçoit plus rien. Ici, on ne décide
// pas : on signale la combinaison (`onFormed`), et c'est le contrôleur de
// session qui prend la manette (« tout à zéro » : touches tenues relâchées,
// six axes au repos, clics et touches de la manette en souris relâchés)
// quand la barre ou le menu s'ouvre vraiment.
//
// Les quatre touches partent au jeu comme les autres ; la combinaison ne se
// forme qu'au bout de 250 ms tenues sans qu'aucune soit lâchée. En dessous
// (un jeu qui les presse ensemble, un appui maladroit), rien ne change pour
// le jeu. Formée, les touches tenues seront relâchées pour l'ordinateur par
// la capture. (Jusqu'à la 0.2.6, le Mac la formait à la quatrième touche,
// qu'il écartait ; les deux apps font maintenant pareil.)
//
// Fils : aucun verrou ici. La minuterie tourne sur la file donnée ; `exclusive`
// entoure son travail pour la plateforme qui protège l'état par un verrou
// (Mac : celui de GamepadGate, le hook de kynput appelant d'un autre fil) ;
// l'Apple TV appelle tout depuis sa file série, sans verrou.

import Foundation
@_implementationOnly import CKyber

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

/// Touche de navigation pendant que la manette pilote l'app.
public enum PadKey: Sendable {
    case left, right, up, down, accept, back
}

final class PadCombo {
    static let buttons: Set<PadButton> = [.leftShoulder, .rightShoulder, .start, .back]
    /// Les six axes à remettre au repos à la capture (kynput ne dit pas leur état).
    static let axes: [PadAxis] = [.leftX, .leftY, .rightX, .rightY, .leftTrigger, .rightTrigger]
    /// Durée tenue avant que la combinaison se forme.
    static let holdDelay: DispatchTimeInterval = .milliseconds(250)

    /// Combinaison formée pour cette manette. Appelé sur la file de la
    /// minuterie, dans `exclusive`.
    var onFormed: ((UInt32) -> Void)?
    /// Réglage « la combinaison ouvre le menu » (Apple TV) ; `false` : rien n'est armé.
    var enabled = true
    /// Entoure le travail de la minuterie (prise du verrou de la plateforme).
    var exclusive: (() -> Void) -> Void = { $0() }

    private let delay: DispatchTimeInterval
    private let queue: DispatchQueue
    /// Manettes vues passer (branchées, ou dont une touche ou un axe est passé).
    private(set) var pads = Set<UInt32>()
    private var pressed: [UInt32: Set<PadButton>] = [:]
    /// Combinaison formée et touches pas toutes relâchées depuis : ses touches
    /// n'agissent pas au relâchement (manette en souris). Oubliée au premier
    /// événement qui suit le relâchement complet, pour que le dernier
    /// relâchement la voie encore formée.
    private var formedPads = Set<UInt32>()
    private var timers: [UInt32: DispatchWorkItem] = [:]

    /// `queue` : file série de la minuterie.
    init(delay: DispatchTimeInterval = PadCombo.holdDelay, on queue: DispatchQueue) {
        self.delay = delay
        self.queue = queue
    }

    /// Touches tenues d'une manette, d'après ce qui est passé par ici.
    func pressed(_ id: UInt32) -> Set<PadButton> { pressed[id] ?? [] }

    /// La combinaison s'est formée et une de ses touches est encore tenue.
    func formed(_ id: UInt32) -> Bool { formedPads.contains(id) }

    /// Manette vue (branchement, axe) : à remettre à zéro à la capture.
    func note(_ id: UInt32) { pads.insert(id) }

    /// Manette débranchée.
    func forget(_ id: UInt32) {
        pads.remove(id)
        pressed[id] = nil
        formedPads.remove(id)
        timers[id]?.cancel()
        timers[id] = nil
    }

    /// Tout oublier (fin de session).
    func reset() {
        for id in Array(pads) { forget(id) }
        pressed = [:]
    }

    /// État réel d'une manette (resynchronisation après une remise à zéro).
    func set(pressed buttons: Set<PadButton>, for id: UInt32) {
        pads.insert(id)
        pressed[id] = buttons
    }

    /// Une touche change : les quatre tenues arment la minuterie, une lâchée
    /// l'annule.
    func button(_ id: UInt32, _ button: PadButton, pressed down: Bool) {
        pads.insert(id)
        if pressed[id, default: []].isDisjoint(with: Self.buttons) { formedPads.remove(id) }
        if down { pressed[id, default: []].insert(button) } else { pressed[id]?.remove(button) }
        guard Self.buttons.contains(button) else { return }
        if down, enabled, pressed[id, default: []].isSuperset(of: Self.buttons) {
            arm(id)
        } else if !down {
            timers[id]?.cancel()
            timers[id] = nil
        }
    }

    private func arm(_ id: UInt32) {
        timers[id]?.cancel()
        let item = DispatchWorkItem { [weak self] in
            guard let self else { return }
            self.exclusive {
                guard self.timers[id] != nil else { return }
                self.timers[id] = nil
                guard self.enabled, self.pressed[id, default: []].isSuperset(of: Self.buttons) else { return }
                self.formedPads.insert(id)
                self.onFormed?(id)
            }
        }
        timers[id] = item
        queue.asyncAfter(deadline: .now() + delay, execute: item)
    }

    /// Croix et A / B pendant que l'app tient la manette.
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
