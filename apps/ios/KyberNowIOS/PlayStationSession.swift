// Une partie avec une console PlayStation sur l'iPhone et l'iPad (textes :
// docs/link-ui.md, « Consoles PlayStation »), le pendant de
// SessionController pour une console : même image (StreamView, VideoToolbox,
// Metal, HDR sur un écran HDR, agrandissement MetalFX), même barre, mêmes
// statistiques et même façon de quitter. Le protocole est dans le cœur
// (`PlayStationPlay`, partagé avec le Mac et l'Apple TV) ; le modèle (comptes,
// consoles, réglages) vient de l'app Mac (`PlayStationStore`), qui crée la
// partie et l'ouvre : ici, `open()` la fait passer en plein écran
// (`AppModel.consoleSession`).
//
// Entrées (docs/playstation.md) :
// - manettes (GameController) : `PlayStationPads`, une DualSense telle
//   quelle, les autres lues comme elle ; vibrations simples (iOS ne donne
//   pas la carte son de la DualSense : pas de vibrations HD) ;
// - la manette à l'écran de l'app (bouton de la barre, retenue pour chaque
//   console), avec les signes PlayStation ;
// - clavier et souris de l'iPad (GameController) traduits en manette par le
//   cœur, comme sur Mac (`PlayStationKeyMouse`) ; une manette les exclut ;
// - le clavier de la console : un champ au-dessus de l'image (clavier du
//   système sur iPhone, clavier branché sur iPad).
//
// La barre s'ouvre par la languette, en glissant depuis le bord haut, ou par
// Options + Create + L1 + R1 ; ouverte, la console reçoit une manette au
// repos (et ni clavier ni souris), la manette la parcourt.

import Combine
import GameController
import KyberKit
import SwiftUI
import UIKit

@MainActor
final class PlayStationSession: ObservableObject {
    enum Phase: Equatable {
        case connecting, streaming
        /// Fin : le texte à montrer (nil : fermée ici).
        case ended(String?)
    }

    /// Ce que la barre propose, dans l'ordre (la manette la parcourt).
    enum BarItem: Equatable { case stats, sound, screenPad, settings, rest, disconnect }

    /// La partie qui s'ouvre (`AppModel` la met en plein écran).
    static var onOpen: ((PlayStationSession) -> Void)?

    /// L'écran de la partie est HDR (celui de l'appareil).
    static var screenIsHDR: Bool { HDRDisplay.isHDR(nil) }

    @Published private(set) var phase: Phase = .connecting
    /// Contenu protégé à l'écran de la console.
    @Published private(set) var cantDisplay = false
    @Published private(set) var barOpen = false
    /// La manette pilote la barre (ouverte par la combinaison).
    @Published private(set) var padMode = false
    @Published private(set) var padFocus = 0
    @Published var statsShown = false
    @Published private(set) var stats: PlayStationPlay.Stats?
    @Published private(set) var lostTotal = 0
    @Published private(set) var repairedTotal = 0
    @Published private(set) var muted = false
    /// Manettes branchées (GameController) : clavier et souris ne comptent plus.
    @Published private(set) var padCount = GCController.controllers().count
    /// La manette à l'écran (retenue pour cette console).
    @Published private(set) var screenPad: Bool
    @Published private(set) var screenPadLayout: ScreenPadLayout = .full
    /// La console a ouvert son clavier : le champ et son texte.
    @Published private(set) var consoleKeyboard = false
    @Published var consoleText = ""
    /// Par où passe la partie : puce « Par le relais », ligne « Chemin ».
    @Published private(set) var path = RouteInfo.direct
    /// Mesures de l'agrandissement (statistiques).
    @Published private(set) var upscale: UpscaleReport?

    let console: PlayStationConsole
    let streamView = StreamView(frame: .zero)
    /// Images par seconde demandées à la console (réglages).
    let requestedFps: Int
    private let address: String
    /// Hors de chez soi : la console jointe par une Station et le relais, tenue
    /// le temps de la partie.
    private var remote: PlayStation.Remote?
    private let registered: String
    private let settings: String
    private var layout: String
    private let onEnd: (PlayStationSession, PlayStationPlay.End?) -> Void
    private var play: PlayStationPlay?
    private let keyMouse = PlayStationKeyMouse()
    private var end: PlayStationPlay.End?
    private var finished = false
    private var resting = false
    /// Le texte que la console a vu en dernier (pour ne pas lui renvoyer le sien).
    private var sentText = ""
    private var statsTask: Task<Void, Never>?
    private var bag: Set<AnyCancellable> = []
    private var observers: [NSObjectProtocol] = []

    private static func screenPadKey(_ c: PlayStationConsole) -> String { "playstation.screenPad.\(c.hostID)" }

    init(console: PlayStationConsole, address: String, remote: PlayStation.Remote? = nil, registered: String, settings: String, layout: String, fullScreen: Bool,
         onFullScreen: @escaping (Bool) -> Void, onEnd: @escaping (PlayStationSession, PlayStationPlay.End?) -> Void) {
        self.console = console
        self.address = address
        self.remote = remote
        self.registered = registered
        self.settings = settings
        self.layout = layout
        self.onEnd = onEnd
        requestedFps = (try? JSONDecoder().decode(PSSettings.self, from: Data(settings.utf8)))?.fps ?? 60
        screenPad = UserDefaults.standard.bool(forKey: Self.screenPadKey(console))
        path = remote?.route ?? .direct
        // Sur iPhone et iPad, la partie est toujours en plein écran.
        _ = (fullScreen, onFullScreen)
    }

    /// Une seule partie à la fois, toujours devant.
    var isKey: Bool { true }

    func bringToFront() {}

    func open() {
        Self.onOpen?(self)
        streamView.touchAsMouse = false
        streamView.onTopEdgeSwipe = { [weak self] in self?.setBar(true) }
        streamView.onTap = { [weak self] in
            guard let self else { return }
            if self.barOpen { self.setBar(false) }
        }
        let play = PlayStationPlay(view: streamView)
        play.onState = { [weak self] in self?.stateChanged($0) }
        play.onMenu = { [weak self] in self?.padCombo() }
        play.onNavigate = { [weak self] in self?.padNavigate($0) }
        play.onKeyboard = { [weak self] in self?.keyboardEvent($0) }
        play.setUpscale((try? JSONDecoder().decode(PSSettings.self, from: Data(settings.utf8)))?.upscaleSettings)
        self.play = play
        PlayStationPads.shared.onChange = { [weak self] n in self?.padsChanged(n) }
        PlayStationPads.shared.start()
        keyMouse.attach(play)
        keyMouse.onShortcut = { [weak self] key in self?.shortcut(key) ?? false }
        observe()
        if !play.start(address: address, registered: registered, settings: settings,
                       hevcDecoder: DecoderCapabilities.current.hevc, layout: layout) {
            stateChanged(.ended(.notPaired))
        }
    }

    // MARK: - Réglages pendant la partie

    func setLayout(_ json: String) {
        layout = json
        play?.setLayout(json)
    }

    func setUpscale(_ u: UpscaleSettings?) { play?.setUpscale(u) }

    func setIntensities(rumble: Int, triggers: Int) { play?.setIntensities(rumble: rumble, triggers: triggers) }

    // MARK: - Barre

    var barItems: [BarItem] { [.stats, .sound, .screenPad, .settings, .rest, .disconnect] }

    /// Ouvre ou ferme la barre (`pad` : ouverte par la manette, qui la pilote).
    /// Ouverte, la console reçoit une manette au repos, ni clavier ni souris.
    func setBar(_ open: Bool, pad: Bool = false) {
        guard open != barOpen else { return }
        if open {
            guard phase == .streaming else { return }
            padFocus = 0
            padMode = pad
            PlayStationPads.shared.releaseScreenPad()
        } else {
            padMode = false
        }
        keyMouse.suspended = open
        play?.setCaptured(open)
        withAnimation(.spring(response: 0.32, dampingFraction: 0.86)) { barOpen = open }
    }

    func activate(_ item: BarItem) {
        switch item {
        case .stats: toggleStats()
        case .sound: toggleMute()
        case .screenPad: toggleScreenPad()
        case .settings: openSettings()
        case .rest: goToRest()
        case .disconnect: close()
        }
    }

    func toggleStats() {
        guard phase == .streaming else { return }
        withAnimation(Theme.easeOut) { statsShown.toggle() }
    }

    func toggleMute() {
        muted.toggle()
        play?.muted = muted
    }

    /// « Manette à l'écran » : retenue pour la console ; affichée, une manette
    /// de plus pour la console.
    func toggleScreenPad() {
        screenPad.toggle()
        UserDefaults.standard.set(screenPad, forKey: Self.screenPadKey(console))
        PlayStationPads.shared.setScreenPad(screenPad && phase == .streaming)
        setBar(false)
    }

    func toggleScreenPadLayout() {
        screenPadLayout = screenPadLayout == .full ? .simple : .full
    }

    /// « Réglages de {nom} » : par-dessus la partie.
    func openSettings() {
        setBar(false)
        AppModel.shared?.openConsoleSettings(console)
    }

    func goToRest() {
        guard case .streaming = phase else { return }
        setBar(false)
        resting = true
        play?.gotoBed()
    }

    // MARK: - Manette

    private func padCombo() {
        guard phase == .streaming else { return }
        setBar(!barOpen, pad: true)
    }

    private func padNavigate(_ key: PadKey) {
        guard barOpen else { return }
        if !padMode, key != .back {
            padFocus = 0
            padMode = true
            return
        }
        let items = barItems
        switch key {
        case .left: padFocus = (padFocus + items.count - 1) % items.count
        case .right: padFocus = (padFocus + 1) % items.count
        case .back: setBar(false)
        case .accept: activate(items[min(padFocus, items.count - 1)])
        case .up, .down: break
        }
    }

    private func padsChanged(_ n: Int) {
        padCount = n
    }

    /// ⌃⌥⌘D, ⌃⌥⌘S au clavier de l'iPad : ceux d'une session.
    private func shortcut(_ key: String) -> Bool {
        switch key {
        case "d": close()
        case "s": toggleStats()
        default: return false
        }
        return true
    }

    // MARK: - Clavier de la console

    private func keyboardEvent(_ e: PlayStationPlay.KeyboardEvent) {
        switch e {
        case let .open(text):
            KyberLog.app.notice("PlayStation : clavier de la console ouvert")
            sentText = text
            consoleText = text
            keyMouse.consoleKeyboard = true
            withAnimation(Theme.easeOut) { consoleKeyboard = true }
        case let .text(text):
            sentText = text
            consoleText = text
        case .closed:
            KyberLog.app.notice("PlayStation : clavier de la console fermé")
            closeKeyboard()
        }
    }

    /// Le champ a changé : le texte entier part à la console.
    func consoleTextChanged() {
        guard consoleKeyboard, consoleText != sentText else { return }
        sentText = consoleText
        play?.keyboardText(consoleText)
    }

    /// Entrée (ou « OK » du clavier de l'écran) : le texte est validé.
    func consoleKeyboardAccept() {
        consoleTextChanged()
        play?.keyboardAccept()
        closeKeyboard()
    }

    /// Échap, ou « Annuler » : le texte est refusé.
    func consoleKeyboardReject() {
        play?.keyboardReject()
        closeKeyboard()
    }

    private func closeKeyboard() {
        keyMouse.consoleKeyboard = false
        withAnimation(Theme.easeOut) { consoleKeyboard = false }
    }

    // MARK: - Mesures

    private func startStats() {
        guard statsTask == nil else { return }
        statsTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard let self, !Task.isCancelled, let play = self.play else { return }
                if let remote = self.remote {
                    let route = remote.route
                    if route != self.path { self.path = route }
                }
                self.upscale = play.upscaleReport
                guard let s = play.stats() else { continue }
                self.lostTotal += s.lost
                self.repairedTotal += s.recovered
                self.stats = s
            }
        }
    }

    // MARK: - Déroulé

    /// Arrière-plan : la console reçoit une manette au repos ; au retour, tout revient.
    private func observe() {
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: UIApplication.willResignActiveNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.play?.setCaptured(true)
                self.keyMouse.suspended = true
                PlayStationPads.shared.releaseScreenPad()
            }
        })
        observers.append(center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.play?.setCaptured(self.barOpen)
                self.keyMouse.suspended = self.barOpen
            }
        })
    }

    private func stateChanged(_ s: PlayStationPlay.State) {
        switch s {
        case .connecting:
            phase = .connecting
        case let .streaming(w, h, hdr):
            KyberLog.app.notice("PlayStation : image \(w)x\(h) \(hdr ? "HDR" : "SDR", privacy: .public)")
            if phase != .streaming {
                withAnimation(Theme.easeOut) { phase = .streaming }
                PlayStationPads.shared.setScreenPad(screenPad)
            }
            startStats()
        case let .cantDisplay(on):
            cantDisplay = on
        case let .ended(e):
            guard !finished else { return }
            end = e
            cantDisplay = false
            closeKeyboard()
            setBar(false)
            statsShown = false
            statsTask?.cancel()
            statsTask = nil
            let text = resting ? nil : PSText.end(e)
            phase = .ended(text)
            stopPlay()
            if text == nil { close() }
        }
    }

    /// Arrêt hors du fil principal (`kn_ps_stop` attend sa fin).
    private func stopPlay() {
        PlayStationPads.shared.stop()
        PlayStationPads.shared.onChange = nil
        keyMouse.detach()
        guard let play else { return }
        self.play = nil
        let remote = self.remote
        self.remote = nil
        DispatchQueue.global(qos: .userInitiated).async {
            play.stop()
            withExtendedLifetime(remote) {}
        }
    }

    /// « Se déconnecter », « Fermer » : la partie s'arrête et l'accueil revient.
    func close() {
        guard !finished else { return }
        finished = true
        statsTask?.cancel()
        observers.forEach { NotificationCenter.default.removeObserver($0) }
        observers = []
        stopPlay()
        streamView.detach()
        onEnd(self, end)
    }
}

#if DEBUG
extension PlayStationSession {
    /// Démonstration au simulateur, sans console (`-screen psSession`) : une
    /// mire à la place de l'image ; `-psDemo bar|stats|keyboard|pad|connecting|ended`.
    func demo(_ what: String?) {
        Self.onOpen?(self)
        streamView.touchAsMouse = false
        streamView.onTopEdgeSwipe = { [weak self] in self?.setBar(true) }
        streamView.onTap = { [weak self] in if self?.barOpen == true { self?.setBar(false) } }
        switch what {
        case "connecting": return
        case "ended":
            phase = .ended(PSText.end(.inUse))
            return
        default: break
        }
        streamView.startTestPattern()
        phase = .streaming
        var st = PlayStationPlay.Stats()
        (st.width, st.height, st.hevc, st.hdr, st.targetKbps, st.fps, st.mbps) = (1920, 1080, true, true, 15000, 59.6, 12.4)
        stats = st
        lostTotal = 3
        repairedTotal = 2
        switch what {
        case "bar": setBar(true)
        case "padbar": setBar(true, pad: true); padFocus = 2
        case "stats": statsShown = true
        case "keyboard":
            consoleText = "Astro"
            consoleKeyboard = true
        case "pad": screenPad = true
        default: break
        }
    }
}
#endif

/// Textes des consoles propres à l'iPhone et à l'iPad (docs/link-ui.md,
/// « Consoles PlayStation »).
extension PSText {
    static var upscaleHelpDevice: String {
        String(localized: "La console envoie du 1080p au plus : l'image est agrandie aux pixels de votre écran par la puce graphique de l'appareil. Coupé de lui-même si cet appareil est trop lent.")
    }
    /// Le clavier de la console, sans clavier branché.
    static var typeYourText: String { String(localized: "Tapez votre texte") }
}
