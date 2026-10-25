// Règles du toucher de la vue de session (iPhone, iPad), sans UIKit :
// vérifiées par apps/ios/tests/screen-pad/run.sh.

import Foundation

enum TouchRules {
    /// Ce que fait un toucher simple sur l'image.
    enum Tap: Equatable {
        /// Le clavier de l'écran est sorti : il se replie, rien ne part vers
        /// l'ordinateur (docs/link-ui.md, « Au toucher »).
        case foldKeyboard
        /// La barre de session est ouverte : elle se ferme.
        case closeBar
        /// La manette à l'écran est affichée : le reste de l'écran ne sert
        /// pas de souris.
        case nothing
        /// Un clic sur l'ordinateur.
        case click
    }

    static func tap(keyboardShown: Bool, barOpen: Bool, touchAsMouse: Bool) -> Tap {
        if keyboardShown { return .foldKeyboard }
        if barOpen { return .closeBar }
        return touchAsMouse ? .click : .nothing
    }
}
