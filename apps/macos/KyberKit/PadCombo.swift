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
// Deux façons de la former, selon ce que la plateforme peut faire :
// - `.immediate` (Mac) : formée à la quatrième touche, qui n'atteint pas
//   l'ordinateur (`button` rend true : le hook écarte le paquet) ;
// - `.after(250 ms, …)` (Apple TV) : les quatre touches partent au jeu
//   comme les autres, la combinaison ne se forme qu'au bout de 250 ms tenues ;
//   en dessous (un jeu qui les presse ensemble, un appui maladroit), rien ne
//   change pour le jeu. Formée, les touches tenues seront relâchées pour
//   l'ordinateur par la capture.
//
// Fils : aucun verrou ici. Le Mac appelle sous le verrou de GamepadGate (le
// hook de kynput), l'Apple TV sur sa file série des entrées, où tourne aussi
// la minuterie.

import Foundation

/// Touche de navigation pendant que la manette pilote l'app.
public enum PadKey: Sendable {
    case left, right, up, down, accept, back
}

final class PadCombo {
    /// Quand la combinaison se forme.
    enum Hold {
        /// À la quatrième touche, qui est écartée.
        case immediate
        /// Après `delay` tenues sans qu'aucune soit lâchée ; la minuterie tourne sur `queue`.
        case after(DispatchTimeInterval, on: DispatchQueue)
    }

    static let buttons: Set<PadButton> = [.leftShoulder, .rightShoulder, .start, .back]
    /// Les six axes à remettre au repos à la capture (kynput ne dit pas leur état).
    static let axes: [PadAxis] = [.leftX, .leftY, .rightX, .rightY, .leftTrigger, .rightTrigger]
    /// Durée tenue sur Apple TV.
    static let holdDelay: DispatchTimeInterval = .milliseconds(250)

    /// Combinaison formée pour cette manette. Appelé dans `button` (`.immediate`)
    /// ou sur la file de la minuterie (`.after`).
    var onFormed: ((UInt32) -> Void)?
    /// Réglage « la combinaison ouvre le menu » (Apple TV) ; `false` : rien n'est armé.
    var enabled = true

    private let hold: Hold
    /// Manettes vues passer (branchées, ou dont une touche ou un axe est passé).
    private(set) var pads = Set<UInt32>()
    private var pressed: [UInt32: Set<PadButton>] = [:]
    /// Combinaison formée et touches pas toutes relâchées depuis : ses touches
    /// n'agissent pas au relâchement (manette en souris). Oubliée au premier
    /// événement qui suit le relâchement complet, pour que le dernier
    /// relâchement la voie encore formée.
    private var formedPads = Set<UInt32>()
    private var timers: [UInt32: DispatchWorkItem] = [:]

    init(hold: Hold) {
        self.hold = hold
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

    /// Une touche change. `true` : elle ne doit pas atteindre l'ordinateur (la
    /// quatrième touche, en `.immediate`).
    @discardableResult
    func button(_ id: UInt32, _ button: PadButton, pressed down: Bool) -> Bool {
        pads.insert(id)
        if pressed[id, default: []].isDisjoint(with: Self.buttons) { formedPads.remove(id) }
        if down { pressed[id, default: []].insert(button) } else { pressed[id]?.remove(button) }
        guard Self.buttons.contains(button) else { return false }
        if down, enabled, pressed[id, default: []].isSuperset(of: Self.buttons) {
            switch hold {
            case .immediate:
                form(id)
                return true
            case .after(let delay, let queue):
                arm(id, after: delay, on: queue)
            }
        } else if !down {
            timers[id]?.cancel()
            timers[id] = nil
        }
        return false
    }

    private func arm(_ id: UInt32, after delay: DispatchTimeInterval, on queue: DispatchQueue) {
        timers[id]?.cancel()
        let item = DispatchWorkItem { [weak self] in
            guard let self, self.timers[id] != nil else { return }
            self.timers[id] = nil
            guard self.enabled, self.pressed[id, default: []].isSuperset(of: Self.buttons) else { return }
            self.form(id)
        }
        timers[id] = item
        queue.asyncAfter(deadline: .now() + delay, execute: item)
    }

    private func form(_ id: UInt32) {
        formedPads.insert(id)
        onFormed?(id)
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
