// Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md ; textes et
// comportement : docs/link-ui.md, « Plusieurs ordinateurs ») : les sessions
// de l'app, une fenêtre chacune, dans l'ordre d'ouverture, quatre au plus,
// jamais deux fois le même ordinateur. La session active est celle de la
// fenêtre au premier plan (la dernière d'entre elles à l'avoir été quand
// l'accueil, les réglages ou une autre app passent devant) : elle seule
// reçoit le clavier, la souris, les manettes, le presse-papiers et le son ;
// « Garder le son » laisse une autre audible. Un seul endroit le dit à chaque
// session : `applyFocus`.

import AppKit
import Combine
import KyberKit

@MainActor
final class SessionManager: ObservableObject {
    /// Quatre ordinateurs au plus en même temps.
    static let limit = 4

    /// Les sessions, dans l'ordre d'ouverture : de la connexion à la fin de
    /// leur fenêtre (un échec affiché compte, jusqu'à « Revenir à l'accueil »).
    @Published private(set) var all: [SessionController] = []
    /// L'ordinateur de la session active.
    @Published private(set) var activeID: UUID?
    /// Ordinateurs des sessions, de la moins récemment active à la plus
    /// récente : la session qui prend la main quand l'active se termine.
    private var recent: [UUID] = []
    /// Sessions revenues (fenêtre fermée) que le cœur n'a pas encore dites
    /// finies (bilan d'Admin) : gardées jusque-là, 10 s au plus, pour leur fin
    /// (notification, fin de l'app).
    private(set) var lingering: [SessionController] = []
    /// La première session a pris la place de l'accueil (fenêtre cachée) :
    /// la dernière le ramène, sauf fermée par sa fenêtre.
    var homeHidden = false
    private var phaseWatchers: [ObjectIdentifier: AnyCancellable] = [:]
    private var keyObserver: NSObjectProtocol?

    init() {
        // La session active suit la fenêtre qui prend le clavier.
        keyObserver = NotificationCenter.default.addObserver(forName: NSWindow.didBecomeKeyNotification, object: nil, queue: .main) { [weak self] note in
            guard let window = note.object as? NSWindow else { return }
            MainActor.assumeIsolated { self?.windowBecameKey(window) }
        }
    }

    var isEmpty: Bool { all.isEmpty }
    var count: Int { all.count }
    /// Quatre sessions : « Se connecter » est inactif ailleurs.
    var isFull: Bool { all.count >= Self.limit }
    /// Les sessions qui tiennent encore (ni finies, ni en échec).
    var live: [SessionController] { all.filter(\.isLive) }
    var active: SessionController? { activeID.flatMap(session(for:)) }

    func session(for hostID: UUID) -> SessionController? {
        all.first { $0.host.id == hostID }
    }

    /// La fenêtre active, pour placer la fenêtre d'une session de plus.
    var anchorWindow: NSWindow? {
        NSApp.keyWindow ?? active?.windows.first ?? NSApp.mainWindow
    }

    /// Une session de plus : sa fenêtre s'ouvre devant, elle devient active.
    func add(_ session: SessionController) {
        all.append(session)
        // Les changements d'état d'une session (icône, menus) passent par le gestionnaire.
        phaseWatchers[ObjectIdentifier(session)] = session.$phase.dropFirst().sink { [weak self] _ in self?.objectWillChange.send() }
        activate(session.host.id)
    }

    /// La session est revenue (fenêtre en train de se fermer) : retirée ; si
    /// elle était active, la plus récente des autres prend la main.
    func remove(_ session: SessionController) {
        all.removeAll { $0 === session }
        phaseWatchers[ObjectIdentifier(session)] = nil
        recent.removeAll { $0 == session.host.id }
        if activeID == session.host.id {
            activeID = recent.last ?? all.last?.host.id
        }
        applyFocus()
        linger(session)
    }

    /// « Garder le son » de la barre ou du menu de l'icône, pour cette session.
    func setKeepSound(_ on: Bool, for session: SessionController) {
        guard session.keepSound != on else { return }
        session.keepSound = on
        KyberLog.app.notice("\(session.host.name, privacy: .public) : garder le son \(on ? "allumé" : "éteint", privacy: .public)")
        applyFocus()
    }

    /// ⌃⌥⌘O, « Ordinateur suivant » : la fenêtre de l'ordinateur suivant,
    /// dans l'ordre d'ouverture, même en plein écran.
    func showNext() {
        guard all.count > 1 else { return }
        let i = all.firstIndex { $0.host.id == activeID } ?? -1
        all[(i + 1) % all.count].bringToFront()
    }

    // MARK: Session active

    private func windowBecameKey(_ window: NSWindow) {
        guard let session = all.first(where: { $0.owns(window) }) else { return }
        activate(session.host.id)
    }

    private func activate(_ id: UUID) {
        recent.removeAll { $0 == id }
        recent.append(id)
        guard activeID != id else { return applyFocus() }
        activeID = id
        applyFocus()
    }

    /// Le point d'application unique : à chaque changement de session active
    /// ou de « Garder le son », chaque session apprend si elle est active et
    /// si on l'entend (inactive sans « Garder le son » : muette). Une app avec
    /// une seule session la garde active et entendue.
    private func applyFocus() {
        for session in all {
            let active = session.host.id == activeID
            session.applyFocus(active: active, muted: !active && !session.keepSound)
        }
    }

    // MARK: Fins

    /// Revenue avant que le cœur la dise finie : gardée 10 s au plus.
    private func linger(_ session: SessionController) {
        guard !session.coreEnded else { return }
        lingering.append(session)
        DispatchQueue.main.asyncAfter(deadline: .now() + 10) { [weak self, weak session] in
            self?.lingering.removeAll { $0 === session }
        }
    }

    /// Finie dans le cœur : plus rien à attendre d'elle.
    func ended(_ session: SessionController) {
        lingering.removeAll { $0 === session }
    }
}
