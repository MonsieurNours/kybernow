// Règles de session sans interface, communes à l'iPhone, l'iPad et l'Apple
// TV (le fichier est aussi compilé dans l'app tvOS) : quand la veille est
// retenue, quand la perte du premier plan relâche tout, quand Retour et la
// combinaison de la manette ouvrent le menu. Vérifiées sans simulateur par
// `apps/ios/tests/session-rules/run.sh`.

#if canImport(UIKit)
import UIKit
#endif

/// Les étapes d'une session d'ordinateur, vues par ces règles (le `Phase`
/// de chaque `SessionController` s'y ramène).
enum SessionStage: Equatable {
    case preparing, connecting, starting, streaming, reconnecting, ending, ended, failed
}

enum SessionRules {
    /// L'appareil ne se met pas en veille de la préparation à la fin, reconnexion
    /// comprise : l'image ne passe ni par `AVPlayer` ni par une vidéo du système,
    /// rien d'autre ne retient la veille (spectateur, manette, vidéo sur le PC).
    static func keepsAwake(_ stage: SessionStage) -> Bool {
        switch stage {
        case .preparing, .connecting, .starting, .streaming, .reconnecting: true
        case .ending, .ended, .failed: false
        }
    }

    /// Perdre le premier plan relâche tout sur l'ordinateur et plafonne le débit
    /// à toute étape vivante, pas seulement pendant l'image (verrouillage pendant
    /// « Reconnexion… » ou juste avant la première image).
    static func reactsToBackground(_ stage: SessionStage) -> Bool {
        keepsAwake(stage)
    }

    /// Apple TV : Retour et la combinaison de la manette ouvrent le menu pendant
    /// l'image et pendant la reconnexion (le menu a « Se déconnecter » : sans lui,
    /// rien ne sortait d'une reconnexion à un PC éteint).
    static func opensMenu(_ stage: SessionStage) -> Bool {
        stage == .streaming || stage == .reconnecting
    }

    /// La manette et la télécommande vont au jeu (Retour compris) seulement
    /// pendant l'image ; pendant la reconnexion, Retour revient à l'app.
    static func gameOwnsRemote(_ stage: SessionStage) -> Bool {
        stage == .streaming
    }
}

/// Qui retient la veille : chaque session (ordinateur, console) s'y inscrit ;
/// la veille revient quand plus aucune ne la tient.
struct WakeHolds {
    private var holders = Set<ObjectIdentifier>()

    /// `true` : au moins une session retient encore la veille.
    @discardableResult
    mutating func set(_ owner: ObjectIdentifier, _ on: Bool) -> Bool {
        if on { holders.insert(owner) } else { holders.remove(owner) }
        return !holders.isEmpty
    }

    var held: Bool { !holders.isEmpty }
}

#if canImport(UIKit)
/// La veille de l'iPhone, de l'iPad ou de l'Apple TV, retenue par les sessions.
@MainActor
enum SleepGuard {
    private static var holds = WakeHolds()

    static func hold(_ owner: AnyObject, _ on: Bool) {
        let held = holds.set(ObjectIdentifier(owner), on)
        if UIApplication.shared.isIdleTimerDisabled != held {
            UIApplication.shared.isIdleTimerDisabled = held
        }
    }
}
#endif
