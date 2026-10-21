// Ce que l'appareil tient enfoncé sur l'ordinateur, et la politique « tout
// relâcher » commune au Mac et à l'Apple TV.
//
// Le protocole kynput est en deltas : un relâchement qui ne part pas laisse
// la touche collée sur l'ordinateur jusqu'à la fin de la session. Trois
// moments où les relâchements ne nous parviendraient plus :
// - la fenêtre du Mac perd le clavier (⌘Tab, ⌘Espace, Spotlight, autre app) :
//   `release(into:)` relâche touches, modificateurs et boutons de souris
//   tenus. Pas de FlushKeys : il relâcherait aussi les manettes, qui, lues
//   par SDL, continuent de parler à l'ordinateur sans la fenêtre et ne
//   renverraient pas leur état ;
// - l'app Apple TV passe en arrière-plan (bouton TV, autre app, veille) : la
//   manette en souris est relâchée (`release`), les manettes remises à zéro
//   (GamepadBridge), puis FlushKeys pour le clavier Bluetooth et tout le
//   reste, l'ordinateur relâchant de son côté ;
// - la fin de session : FlushKeys, et tout est oublié (`forget`).
//
// Au retour (clavier retrouvé sur Mac ; entrées reliées de nouveau après une
// relance de l'image, sur les deux : le service d'entrées de Kyber est neuf
// et ne tient plus rien), les modificateurs réellement tenus sont
// resynchronisés (`resyncModifiers`) : ceux que nous ne comptons pas encore
// repartent, du côté réellement tenu. Les autres touches et les boutons ne
// sont pas renvoyés : un jeu y verrait un nouvel appui.
//
// Fils : aucun verrou ; chaque plateforme l'appelle là où elle tient ses
// entrées (fil principal sur Mac, file des entrées sur Apple TV).

import Foundation

final class ReleaseAll {
    /// Scancodes partis en appui, pas encore en relâchement.
    private(set) var scancodes = Set<UInt16>()
    private(set) var mouseButtons = Set<InputPipeline.MouseButton>()
    /// Touches de modification tenues, dans l'identifiant de la plateforme
    /// (code kVK sur Mac, usage HID avec GameController) : ce qu'il faut pour
    /// les resynchroniser.
    private(set) var modifiers = Set<UInt16>()

    func key(scancode: UInt16, pressed: Bool) {
        if pressed { scancodes.insert(scancode) } else { scancodes.remove(scancode) }
    }

    func mouseButton(_ button: InputPipeline.MouseButton, pressed: Bool) {
        if pressed { mouseButtons.insert(button) } else { mouseButtons.remove(button) }
    }

    /// `true` : l'état a changé (touche nouvellement tenue, ou relâchée alors
    /// qu'elle l'était) ; `false` : rien à envoyer.
    @discardableResult
    func modifier(_ id: UInt16, pressed: Bool) -> Bool {
        pressed ? modifiers.insert(id).inserted : modifiers.remove(id) != nil
    }

    /// Relâche sur l'ordinateur tout ce qui est tenu : boutons de souris, puis
    /// touches (modificateurs compris, par leur scancode), et oublie tout.
    func release(into input: InputPipeline?) {
        for button in mouseButtons { input?.mouseButton(button, pressed: false) }
        for scancode in scancodes { input?.key(scancode: scancode, pressed: false) }
        forget()
    }

    /// Plus rien n'est tenu (fin de session, après FlushKeys).
    func forget() {
        scancodes.removeAll()
        mouseButtons.removeAll()
        modifiers.removeAll()
    }

    /// Les modificateurs ne sont plus tenus sur l'ordinateur (service
    /// d'entrées relancé) : à resynchroniser.
    func forgetModifiers() {
        modifiers.removeAll()
    }

    /// Modificateurs réellement tenus à cet instant : ceux que nous ne
    /// comptons pas encore repartent en appui (`send`, qui doit aussi noter le
    /// scancode par `key(scancode:pressed:)`).
    func resyncModifiers(held: [UInt16], send: (UInt16) -> Void) {
        for id in held where modifiers.insert(id).inserted {
            send(id)
        }
    }
}
