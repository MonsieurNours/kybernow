// Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md ; textes et
// comportement : docs/link-ui.md, « Plusieurs ordinateurs ») : les sessions
// de l'app, une fenêtre chacune, dans l'ordre d'ouverture, quatre au plus,
// jamais deux fois le même ordinateur. « Qui est active » a une seule source :
// le registre des sessions du cœur (`kn_focus_*`, par `SessionEngine`), qui
// coupe ou rend lui-même les entrées, le son, le presse-papiers et les
// manettes de chaque session. L'app lui dit seulement quelle fenêtre passe au
// premier plan, « Garder le son » et « Ordinateur suivant », puis relit son
// état pour l'interface (`refresh`).

import AppKit
import Combine
import KyberKit

@MainActor
final class SessionManager: ObservableObject {
    /// Les sessions, dans l'ordre d'ouverture : de la connexion à la fin de
    /// leur fenêtre (un échec affiché compte, jusqu'à « Fermer »).
    @Published private(set) var all: [SessionController] = []
    /// L'ordinateur de la session active, lu dans le registre du cœur.
    @Published private(set) var activeID: UUID?
    /// Sessions revenues (fenêtre fermée) que le cœur n'a pas encore dites
    /// finies (bilan d'Admin) : gardées jusque-là, 10 s au plus, pour leur fin
    /// (notification, fin de l'app).
    private(set) var lingering: [SessionController] = []
    /// La première session a pris la place de l'accueil (fenêtre cachée) :
    /// la dernière le ramène, sauf fermée par sa fenêtre.
    var homeHidden = false
    private var phaseWatchers: [ObjectIdentifier: AnyCancellable] = [:]
    private var keyObserver: NSObjectProtocol?
    #if DEBUG
    /// Démonstration : ses sessions n'ont pas de session du cœur, donc pas de
    /// registre ; la dernière fenêtre de session à avoir eu le clavier est
    /// active (et « Garder le son » reste dans l'interface).
    private var demoRecent: [UUID] = []
    /// `-demoCore YES` : de vraies sessions du cœur, donc le registre.
    private let demoCore = UserDefaults.standard.bool(forKey: "demoCore")
    #endif

    init() {
        // La fenêtre qui prend le clavier : sa session devient active dans le
        // registre, tout de suite (sans file : pendant la notification même).
        // AppKit rend la fenêtre clé avant de lui passer le clic qui l'a
        // activée : ce clic part ainsi vers son ordinateur, la session déjà
        // active, comme avec une seule session (`acceptsFirstMouse` de StreamView).
        keyObserver = NotificationCenter.default.addObserver(forName: NSWindow.didBecomeKeyNotification, object: nil, queue: nil) { [weak self] note in
            guard let window = note.object as? NSWindow else { return }
            MainActor.assumeIsolated { self?.windowBecameKey(window) }
        }
    }

    var isEmpty: Bool { all.isEmpty }
    var count: Int { all.count }
    /// Quatre ordinateurs au plus (`kn_focus_max`) : « Se connecter » est
    /// inactif ailleurs (« Quatre ordinateurs au plus en même temps. »).
    var isFull: Bool {
        #if DEBUG
        if AppModel.demo, !demoCore { return all.count >= SessionEngine.focusMax }
        #endif
        return SessionEngine.focusCount >= SessionEngine.focusMax
    }
    /// Les sessions qui tiennent encore (ni finies, ni en échec).
    var live: [SessionController] { all.filter(\.isLive) }
    var active: SessionController? { activeID.flatMap(session(for:)) }

    /// Les commandes du menu Session et leurs raccourcis : la session de la
    /// fenêtre clé (même une session en échec, hors registre), sinon la
    /// session active du registre.
    var commandTarget: SessionController? {
        NSApp.keyWindow.flatMap { window in all.first { $0.owns(window) } } ?? active
    }

    func session(for hostID: UUID) -> SessionController? {
        all.first { $0.host.id == hostID }
    }

    /// La fenêtre active, pour placer la fenêtre d'une session de plus.
    var anchorWindow: NSWindow? {
        NSApp.keyWindow ?? active?.windows.first ?? NSApp.mainWindow
    }

    /// Une session de plus, sa fenêtre ouverte devant (et inscrite au registre
    /// du cœur si elle a pu partir).
    func add(_ session: SessionController) {
        all.append(session)
        // Ses changements d'état (fin, échec : désinscrite du registre) redessinent l'icône et les menus.
        phaseWatchers[ObjectIdentifier(session)] = session.$phase.dropFirst().receive(on: DispatchQueue.main).sink { [weak self] _ in
            self?.refresh()
        }
        #if DEBUG
        if AppModel.demo, !demoCore { demoRecent.append(session.host.id) }
        #endif
    }

    /// La session est revenue (fenêtre en train de se fermer) : retirée
    /// (le cœur l'a déjà désinscrite à sa fin).
    func remove(_ session: SessionController) {
        all.removeAll { $0 === session }
        phaseWatchers[ObjectIdentifier(session)] = nil
        #if DEBUG
        demoRecent.removeAll { $0 == session.host.id }
        #endif
        refresh()
        linger(session)
    }

    /// « Garder le son » de la barre ou du menu de l'icône, pour cette session.
    func setKeepSound(_ on: Bool, for session: SessionController) {
        session.setKeepSound(on)
        refresh()
    }

    /// ⌃⌥⌘O, « Ordinateur suivant » : la session ouverte après l'active
    /// (registre du cœur), sa fenêtre devant, même en plein écran.
    func showNext() {
        guard all.count > 1 else { return }
        var next = SessionEngine.focusNextID.flatMap { id in all.first { $0.focusID == id } }
        #if DEBUG
        if AppModel.demo, !demoCore {
            let i = all.firstIndex { $0.host.id == activeID } ?? -1
            next = all[(i + 1) % all.count]
        }
        #endif
        guard let next else { return }
        next.bringToFront()
        activate(next)
    }

    // MARK: Session active

    private func windowBecameKey(_ window: NSWindow) {
        guard let session = all.first(where: { $0.owns(window) }) else {
            // L'accueil ou les réglages : le menu Session vise de nouveau la session active.
            if !all.isEmpty { objectWillChange.send() }
            return
        }
        activate(session)
    }

    private func activate(_ session: SessionController) {
        session.focusActivate()
        #if DEBUG
        if AppModel.demo, !demoCore {
            demoRecent.removeAll { $0 == session.host.id }
            demoRecent.append(session.host.id)
        }
        #endif
        refresh()
    }

    /// Relit le registre du cœur (la seule source) et donne à chaque session
    /// son état pour l'interface : active ou non (trait de repère, barre,
    /// manette de la barre), « Garder le son ». Après chaque inscription,
    /// activation, « Garder le son », fin de session.
    func refresh() {
        var active = SessionEngine.focusActiveID.flatMap { id in all.first { $0.focusID == id } }
        #if DEBUG
        if AppModel.demo, !demoCore { active = demoRecent.last.flatMap(session(for:)) }
        #endif
        if activeID != active?.host.id { activeID = active?.host.id }
        for session in all { session.focusChanged(active: session === active) }
        objectWillChange.send()
    }

    // MARK: Fins

    /// Revenue avant que le cœur la dise finie : gardée 10 s au plus. Sans
    /// session du cœur (départ refusé, mot de passe introuvable), rien à attendre.
    private func linger(_ session: SessionController) {
        guard session.coreStarted, !session.coreEnded else { return }
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
