// Pointeur de l'ordinateur et mode de la souris : ce que les vues de session
// du Mac et de l'Apple TV ont en commun. Les vues ne font plus que dessiner
// (NSCursor et couche sur Mac, couche sur Apple TV) et capter les événements
// de leur plateforme (AppKit, GameController, et demain le toucher sur iOS).
//
// Pointeur : l'ordinateur annonce la forme, le point chaud, la position et la
// visibilité de son pointeur (kyber-patches 0006 : positions seules sans
// bitmap quand la forme ne change pas). L'Apple TV tient en plus une position
// à elle (télécommande, manette ou souris Bluetooth en positions absolues :
// Windows masque son pointeur sans souris branchée et ne le réaffiche que sur
// une position absolue) ; le Mac n'en a pas besoin, sa souris est celle du
// système.
//
// Mode de la souris : Bureau (positions absolues, pointeur de l'appareil),
// Jeu (déplacements bruts), Auto (jeu quand un jeu est en plein écran sur
// l'ordinateur, d'après la veille de la console). Sur Mac, Auto demande en
// plus que le jeu cache son pointeur (vue subjective), avec une tolérance de
// 0,6 s pour un pointeur remontré un instant et un recontrôle 0,7 s après ;
// sur Apple TV le plein écran suffit (la souris Bluetooth passe en brut).
//
// Fils : l'état est tenu sur une file série donnée à la création. Sur Apple
// TV, c'est la file des entrées de GameController (`InputQueue.device`) : la
// position du pointeur en mode bureau n'y passe plus par le fil principal, et
// clics et molette restent ordonnés derrière les déplacements. Sur Mac, c'est
// le fil principal : AppKit y livre déjà la souris, et le mode de la souris
// dépend de la fenêtre. Le dessin remonte toujours sur le fil principal
// (`onChanged`, à la charge de la vue).

import CoreGraphics
import Foundation

/// File série unique des entrées de l'appareil hors du fil principal
/// (GameController sur Apple TV et iOS : manettes, télécommande, clavier et
/// souris Bluetooth, et le pointeur tenu par l'app).
public enum InputQueue {
    public static let device = DispatchQueue(label: "fr.leslignesclaires.kybernow.manettes", qos: .userInteractive)
}

/// Bureau : souris absolue, pointeur de l'appareil toujours là. Jeu : souris
/// relative (déplacements bruts). Auto : jeu quand un jeu est en plein écran
/// sur l'ordinateur (et, sur Mac, cache son pointeur).
public enum MouseMode: Sendable { case auto, desktop, game }

/// Règles Auto / Bureau / Jeu, avec la veille du plein écran de la console.
struct MouseModeRules {
    /// Mac : en Auto, le jeu doit aussi cacher son pointeur ; Apple TV : le
    /// plein écran suffit.
    let needsHiddenPointer: Bool

    /// Un pointeur remontré moins longtemps que ça (menus, clignotement de
    /// Windows) ne fait pas quitter le mode jeu…
    static let shownTolerance: TimeInterval = 0.6
    /// … et on revérifie après ce délai.
    static let recheckDelay: TimeInterval = 0.7

    /// La souris doit-elle être relative. `visibleFor` : depuis combien de
    /// temps l'ordinateur montre son pointeur (0 : caché).
    func wantsRelative(mode: MouseMode, gameFullscreen: Bool, remoteHidden: Bool, visibleFor: TimeInterval, relativeActive: Bool) -> Bool {
        switch mode {
        case .desktop: return false
        case .game: return true
        case .auto:
            guard gameFullscreen else { return false }
            guard needsHiddenPointer else { return true }
            return relativeActive ? visibleFor < Self.shownTolerance : remoteHidden
        }
    }
}

/// État du pointeur de l'ordinateur, tenu sur une file série.
final class PointerLayer {
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
        /// Depuis quand il le montre (nil : caché).
        var remoteVisibleSince: TimeInterval? = 0
        /// Dernière forme reçue (un masquage ou une forme vide la garde).
        var shape: Shape?
        /// Position tenue par l'appareil (Apple TV), à défaut celle de l'ordinateur.
        var held: CGPoint?
        var position: CGPoint? { held ?? remotePosition }
        /// Depuis combien de temps l'ordinateur montre son pointeur (0 : caché).
        var visibleFor: TimeInterval {
            remoteVisibleSince.map { ProcessInfo.processInfo.systemUptime - $0 } ?? 0
        }
    }

    /// Ce qui a changé ; dans tous les cas, à redessiner.
    struct Change {
        /// L'ordinateur a caché ou remontré son pointeur.
        var visibility = false
        /// Nouvelle forme.
        var shape = false
    }

    /// File de l'état ; `onChanged` y est appelé.
    let queue: DispatchQueue
    /// L'appareil tient une position à lui (Apple TV) ; sinon seule celle de
    /// l'ordinateur compte (Mac).
    let holdsPosition: Bool
    /// Sur la file seulement.
    private(set) var state = State()
    /// Écran de l'ordinateur (pixels) ; la position tenue y reste. Sur la file.
    private(set) var hostSize: CGSize
    /// Envoi d'une position absolue à l'ordinateur (InputPipeline.mouse). Sur la file.
    var send: ((Int16, Int16) -> Void)?
    /// L'état a changé (sur la file) : la vue redessine sur le fil principal.
    var onChanged: ((State, Change) -> Void)?
    /// Dernier déplacement de la position tenue : l'ordinateur ne reprend la
    /// main sur elle qu'au repos de nos entrées.
    private var lastLocalMove = 0.0
    private static let localMoveWindow = 0.4

    init(queue: DispatchQueue, holdsPosition: Bool, hostSize: CGSize = CGSize(width: 1920, height: 1080)) {
        self.queue = queue
        self.holdsPosition = holdsPosition
        self.hostSize = hostSize
    }

    /// Nouvelle chaîne d'entrées (début ou relance de l'image) : l'écran de
    /// l'ordinateur et l'envoi des positions, sans toucher à la position tenue.
    func attach(hostSize: CGSize, send: @escaping (Int16, Int16) -> Void) {
        onQueue()
        self.hostSize = hostSize
        self.send = send
    }

    /// Vérifie (Debug) qu'on est bien sur la file.
    @inline(__always) private func onQueue() {
        #if DEBUG
        dispatchPrecondition(condition: .onQueue(queue))
        #endif
    }

    /// Annonce de l'ordinateur (depuis le gestionnaire de kynput, par la file).
    func remoteCursorChanged(_ remote: RemoteCursor) {
        onQueue()
        state.remotePosition = remote.position
        var change = Change()
        if remote.visible == state.remoteHidden {
            state.remoteHidden = !remote.visible
            KyberLog.app.notice("pointeur de l'ordinateur \(remote.visible ? "visible" : "caché", privacy: .public)")
            state.remoteVisibleSince = state.remoteHidden ? nil : ProcessInfo.processInfo.systemUptime
            change.visibility = true
        }
        // L'ordinateur a déplacé son pointeur lui-même (au repos de nos
        // entrées) : la position tenue repart de la sienne.
        if holdsPosition, ProcessInfo.processInfo.systemUptime - lastLocalMove > Self.localMoveWindow {
            state.held = remote.position
        }
        // Forme nouvelle : on l'adopte. Masquage ou forme vide : on garde la dernière.
        if let image = remote.image {
            state.shape = Shape(image: image, hotSpot: remote.hotSpot)
            change.shape = true
        }
        onChanged?(state, change)
    }

    /// L'appareil déplace le pointeur (pixels de l'écran de l'ordinateur) et
    /// envoie sa position (Apple TV : télécommande, manette, souris Bluetooth
    /// en mode bureau). Il part du dernier pointeur connu, ou du centre.
    func move(dx: Double, dy: Double) {
        onQueue()
        guard holdsPosition, hostSize.width > 0, hostSize.height > 0 else { return }
        var p = state.position ?? CGPoint(x: hostSize.width / 2, y: hostSize.height / 2)
        p.x = min(max(0, p.x + dx), hostSize.width - 1)
        p.y = min(max(0, p.y + dy), hostSize.height - 1)
        state.held = p
        lastLocalMove = ProcessInfo.processInfo.systemUptime
        sendHeld()
        onChanged?(state, Change())
    }

    /// L'écran de l'ordinateur a changé de taille en cours de session : la
    /// position tenue garde sa place relative (le centre reste au centre), et
    /// l'ordinateur l'apprend tout de suite.
    func setHostSize(_ size: CGSize) {
        onQueue()
        let old = hostSize
        hostSize = size
        if let p = state.held, old.width > 0, old.height > 0 {
            state.held = CGPoint(x: min(max(0, p.x * size.width / old.width), size.width - 1),
                                 y: min(max(0, p.y * size.height / old.height), size.height - 1))
            sendHeld()
        }
        onChanged?(state, Change())
    }

    /// Entrées reliées de nouveau à l'ordinateur (service d'entrées relancé) :
    /// la position tenue est renvoyée à sa place.
    func resendHeld() {
        onQueue()
        sendHeld()
    }

    private func sendHeld() {
        guard let p = state.held else { return }
        send?(Int16(clamping: Int(p.x.rounded())), Int16(clamping: Int(p.y.rounded())))
    }

    /// Fin de session : la position tenue et la visibilité sont oubliées, la
    /// forme reste (la prochaine session la remplace).
    func reset() {
        onQueue()
        state.held = nil
        state.remoteHidden = false
        state.remoteVisibleSince = 0
        onChanged?(state, Change())
    }

    // MARK: - Dessin

    /// Facteur d'échelle d'une forme pour la dessiner dans une case de `box`
    /// points. Windows dessine ses pointeurs dans une case de 32 pixels à
    /// 100 %, 48 à 150 %, 64 à 200 %… : ramener cette case à `box` donne la
    /// même taille quelle que soit la mise à l'échelle de Windows ou de
    /// l'appareil ; pointe et image suivent le même facteur. Mac : 32 points,
    /// jamais agrandi (la taille d'un pointeur du Mac) ; Apple TV : 48 points,
    /// jusqu'à deux fois (lisible à 3 mètres).
    static func scale(pixels: CGSize, box: CGFloat, maxFactor: CGFloat) -> CGFloat {
        min(maxFactor, box / max(pixels.width, pixels.height, 1))
    }

    /// Cadre du pointeur dessiné : `local` est la pointe (point actif) dans la
    /// vue, y vers le bas ; `flippedHeight` : hauteur de la vue si son y va
    /// vers le haut (Mac).
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
