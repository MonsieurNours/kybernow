// Une session avec un ordinateur, vue de l'interface : la fenêtre (plein
// écran, passage, composition), la barre au survol, la manette qui la
// pilote, les capsules, les mesures. Le déroulé lui-même (préparation par la
// console, connexion à Kyber, choix de l'écran, relances, reconnexions,
// signal de vie, fin vérifiée) est dans le cœur Rust, par `SessionEngine`
// (docs/coeur-apple.md) : le contrôleur lui donne les réglages et la vue,
// et traduit ses états, ses avis et ses mesures pour SwiftUI.
//
// Chaque session a sa fenêtre (docs/plusieurs-ordinateurs.md). La première
// s'ouvre à la place de l'accueil : le passage (Passage.swift) ouvre
// l'accueil photographié en deux sur l'image, la fenêtre passe en plein écran
// (ou prend tous les écrans), et les moitiés se referment à la fin. Chaque
// session suivante s'ouvre dans une fenêtre titrée du nom de l'ordinateur, par-
// dessus, sur l'écran de la fenêtre active, avec un seul écran (« Cet écran »,
// à la taille de la fenêtre) ; écrans, cadre et mode viennent du cœur
// (`SessionEngine.screensForExtraSession`, `extraSessionWindow`). Avec
// plusieurs écrans (docs/multi-ecrans.md), chaque autre écran de ce Mac a sa
// fenêtre plein écran (SessionScreens.swift) ; la fenêtre de la session est
// sur l'écran principal, qui montre la connexion ; une seule barre, posée sur
// l'écran où le geste a lieu. Chaque session s'inscrit au registre des
// sessions du cœur, qui la dit active ou non ; le gestionnaire
// (Sessions.swift) lui en donne l'état pour l'interface (`focusChanged`).

import AppKit
import Combine
import KyberKit
import SwiftUI

@MainActor
final class SessionController: NSObject, ObservableObject {
    enum Phase: Equatable {
        case preparing, connecting, starting, streaming
        case reconnecting(Int)
        case ending
        case ended
        case failed(String)
    }

    /// Vérifications affichées en fin de session.
    struct Closing: Equatable {
        var displayRestored: Bool?
        var audioRestored: Bool?
        var error: String?
        var done = false
    }

    @Published private(set) var phase: Phase = .preparing
    /// Instant où l'image est apparue (gerbe de particules).
    @Published private(set) var revealedAt: Date?
    @Published private(set) var barVisible = false
    /// La manette pilote la barre (L1 + R1 + START + SELECT).
    @Published private(set) var padMode = false
    /// Élément de la barre qui a le focus en mode manette.
    @Published private(set) var padFocus = 0
    /// Éléments de la barre en mode manette : profil, souris, plein écran
    /// (absent avec plusieurs écrans), déconnexion.
    /// Un invité n'a que la qualité de l'image, le plein écran (absent avec
    /// plusieurs écrans) et « Quitter le partage ».
    var padItems: Int { isGuest ? (canToggleFullScreen ? 3 : 2) : (canToggleFullScreen ? 4 : 3) }
    @Published private(set) var closing = Closing()
    @Published private(set) var host: Host
    @Published private(set) var modeLabel = ""
    /// Mode de l'écran de la session (celui obtenu de l'ordinateur).
    private var streamMode = (width: 0, height: 0, refresh: 0)
    /// Mode demandé à la console (celui de ce Mac) : « Comme cet écran » le
    /// compare à l'écran qui montre la session.
    private var requested: ScreenMode?
    /// La session a un écran virtuel (choix d'écran impossible dans la barre).
    private var virtualDisplay = true
    /// Mesures de l'image (décodage par l'app), une fois par seconde.
    @Published private(set) var stats: VideoStats?
    /// Latences des dernières secondes (courbe de la barre).
    @Published private(set) var latencyHistory: [Double] = []
    /// Mesures des 30 dernières secondes (panneau des mesures).
    @Published private(set) var statsHistory: [VideoStats] = []
    /// Pertes depuis le début de la session.
    @Published private(set) var sessionLosses = (frames: 0, packets: 0)
    /// Panneau des mesures détaillées, en bas à droite (⌃⌥⌘S).
    @Published private(set) var statsPanel = false
    /// Bouton de fermeture du panneau, dans le repère de la vue de session :
    /// seul endroit du panneau qui prend les clics.
    var statsCloseRect: CGRect = .zero
    /// Souris en mode jeu en ce moment.
    @Published private(set) var mouseCaptured = false
    /// L'app est au premier plan (le trait de repère de la barre n'a de sens
    /// que là) ; suivi par les notifications d'activation.
    @Published private(set) var appActive = true
    /// Trait de repère allumé dans l'image en ce moment.
    private var markerShown = false
    /// Jeu en plein écran sur l'ordinateur, d'après la veille de la console.
    @Published private(set) var pcGame = false
    @Published private(set) var audioStats: AudioStats?
    /// Dernier recours de la connexion : la Station redémarre (« La Station
    /// redémarre… » à la place des étapes, jusqu'à la phase suivante).
    @Published private(set) var stationRestarting = false
    /// Rappel affiché quelques secondes, sur un événement de la session (jamais à l'arrivée de l'image).
    enum Hint: Equatable { case mouse, fallback(String), notice(String) }
    /// Profil Bureau, écran de l'ordinateur passé à un autre mode : « Remettre
    /// 3840 × 2160 » dans la barre (le cœur le propose et le retire).
    @Published private(set) var modeRestoreOffer: String?
    /// Rappel affiché quelques secondes en haut de l'écran.
    @Published private(set) var hint: Hint?
    private var mouseHintShown = false

    /// Mode d'un écran : définition réelle et fréquence.
    struct ScreenMode: Equatable {
        let width: Int
        let height: Int
        let refresh: Int
    }

    /// « Comme cet écran », écran de ce Mac changé en session (écran externe
    /// branché ou débranché, définition changée) : mode proposé pour l'écran
    /// de l'ordinateur, jamais appliqué sans l'accord de l'utilisateur
    /// (capsule, puis puce dans la barre).
    @Published private(set) var screenOffer: ScreenMode?
    /// Capsule de la proposition, en haut de l'image (quelques secondes).
    @Published private(set) var screenOfferShown = false
    /// Capsule de la proposition, dans le repère de la vue de session : seul
    /// endroit cliquable hors de la barre.
    var screenOfferRect: CGRect = .zero
    private var screenObservers: [NSObjectProtocol] = []
    private var activeObservers: [NSObjectProtocol] = []
    private var screenCheck: Task<Void, Never>?
    private var screenOfferHide: Task<Void, Never>?
    /// Plein écran recalé sur l'écran restant, en cours.
    private var refitting = false
    /// Mode demandé avant une adaptation, à reprendre si elle est refusée.
    private var requestedBeforeAdapt: ScreenMode?

    /// La session s'est arrêtée sans « Se déconnecter » : Admin ou un autre
    /// appareil y a mis fin, ou une erreur (`endError`). Dit par une
    /// notification quand l'app n'est pas au premier plan.
    private(set) var endedByComputer = false
    private(set) var endError: String?
    /// Appelé une fois la session finie dans le cœur (`ended`) : Admin a dit la remise en état.
    var onEnded: ((SessionController) -> Void)?
    /// La session est finie dans le cœur (`onEnded` est passé).
    private(set) var coreEnded = false

    /// Où s'ouvre la fenêtre de la session (docs/plusieurs-ordinateurs.md).
    enum Opening {
        /// La première session : à la place de l'accueil (photographié pour le
        /// passage s'il est à l'écran), puis plein écran ou tous les écrans,
        /// selon les réglages de l'ordinateur.
        case first(home: NSWindow?)
        /// Une session de plus : une fenêtre sur l'écran de `anchor` (la
        /// fenêtre active), centrée, aux deux tiers de l'écran en 16:9, avec un
        /// seul écran à la taille de la fenêtre, comme toute session fenêtrée.
        case next(anchor: NSWindow?)
    }

    let opening: Opening
    var isFirst: Bool {
        if case .first = opening { return true }
        return false
    }

    // MARK: KyberNow Share

    /// KyberNow Share, session d'invité : la Station qui a accepté le code.
    /// Un invité n'a ni compte, ni profil, ni écran virtuel ; son ordinateur
    /// n'est pas dans la liste de cet appareil (`host` n'est qu'un nom et une
    /// adresse), rien n'est gardé après le partage (docs/kybernow-share.md).
    let guest: ShareFound?
    var isGuest: Bool { guest != nil }
    /// Le rôle de l'invité (« Spectateur », « Joueur »), changé par l'hôte en cours de partage.
    @Published private(set) var shareRole: ShareRole = .spectator
    /// La « Qualité de l'image » de l'invité (celle demandée, changée dans la
    /// barre) et l'image qu'il reçoit (`on_share_image`) ; la barre montre
    /// « 1080p · 60 Hz » quand le cœur sait la dire (`shareImageLabel`).
    @Published private(set) var shareQuality: ShareQuality = .auto
    @Published private(set) var shareImage: ShareImage?
    /// La qualité reçue dans la barre, ou la qualité choisie tant que la taille n'est pas sue.
    var shareImageLabel: String { GuestQuality.barLabel(image: shareImage, quality: shareQuality) }
    /// La raison de la fin de la session d'invité par le partage (« {hôte} a
    /// arrêté le partage. ») : elle reste à l'écran, jamais « Reprendre la main ».
    @Published private(set) var shareEndText: String?
    /// Le partage de cette session d'hôte (« Partager » dans la barre) :
    /// propre à chaque session, comme la barre qui le montre.
    let share: ShareHostModel

    /// Le titre de la session : le nom de l'ordinateur, ou « Invité sur {nom} ».
    var title: String { guest.map { ShareText.guestOn($0.computer) } ?? host.name }

    /// « Partager » avec cet ordinateur : disponible, ou inactif avec la raison
    /// (d'après ce que la Station dit d'elle-même, relu à chaque sondage).
    var shareAvailability: ShareAvailability { share.availability }

    /// Session active : sa fenêtre est (ou a été la dernière) au premier
    /// plan. Elle seule reçoit le clavier, la souris, les manettes, le
    /// presse-papiers et le son (le registre du cœur s'en charge) ; lu dans le
    /// registre par le gestionnaire (`focusChanged`), pour l'interface.
    @Published private(set) var focusActive = true
    /// « Garder le son » : la session reste audible même quand elle n'est
    /// pas active ; éteint au départ, pour cette session (registre du cœur).
    @Published private(set) var keepSound = false
    /// La fenêtre d'une session de plus, donnée par le cœur (`start`).
    private var extraWindow: SessionEngine.ExtraWindow?
    /// La session s'est terminée par la fermeture de sa fenêtre (bouton rouge,
    /// ⌘W) : la dernière ne ramène pas l'accueil, l'app reste dans l'icône.
    private(set) var closedByWindow = false
    /// La session du cœur a démarré : sa fin (`onEnded`) viendra.
    var coreStarted: Bool { engine?.hasSession == true }

    /// La session tient encore (ni finie, ni en échec).
    var isLive: Bool {
        switch phase {
        case .ended, .failed: return false
        default: return true
        }
    }

    private let store: HostStore
    private let onFinish: (SessionController) -> Void

    private let streamView = StreamView(frame: .zero)
    private let scene = PassageScene(frame: .zero)
    /// L'accueil photographié à l'ouverture de la première session : les
    /// moitiés du passage (sans photographie, elles ont le fond du passage).
    private var homeImage: CGImage?
    private let windowDelegate = SessionWindowDelegate()
    /// Barre, rappels, panneaux (SwiftUI), par-dessus l'image. Retirée de la
    /// composition quand elle n'a rien à montrer : l'image peut aller
    /// directement à l'écran (présentation directe en plein écran).
    private var chrome: ChromeHostingView?
    private var compositing: AnyCancellable?
    /// Le passage (accueil ouvert en deux) sort de la composition une fois ouvert.
    private var sceneHide: Task<Void, Never>?
    /// Fenêtre occultée (autre espace, réduite, recouverte) : rendu arrêté, débit baissé.
    private var occlusionObserver: NSObjectProtocol?
    private var occluded = false
    private static let occludedBitrate: UInt32 = 20_000_000
    /// Délai entre le démarrage du flux et l'apparition : le décodeur et la
    /// synchronisation du son se calent, les premières images irrégulières
    /// restent sous l'écran d'attente.
    private static let revealDelay: TimeInterval = 0.9
    private var window: NSWindow?
    private var engine: SessionEngine?
    /// Le cœur dit l'image en cours (l'apparition attend `revealDelay`).
    private var engineStreaming = false
    /// Minuteurs de la barre au pointeur : l'ouverture (le pointeur reste
    /// contre le bord haut) et la fermeture (le pointeur a quitté la barre).
    private var openBar: Task<Void, Never>?
    private var hideBar: Task<Void, Never>?
    /// Écrans de l'ordinateur, pour le choix dans la barre (sans écran virtuel).
    @Published private(set) var screens: [KyberDisplay] = []

    // MARK: Multi-écrans

    /// La surface de la fenêtre de la session, et celles des autres écrans de
    /// ce Mac pris par la session (leurs fenêtres sont à la session).
    private var mainSurface: ScreenSurface?
    private var extraSurfaces: [ScreenSurface] = []
    /// Les écrans de ce Mac tels qu'ils ont été donnés au cœur (l'index d'un
    /// écran dans cette liste est celui du cœur), et leur description.
    private var deviceScreens: [NSScreen] = []
    private var screensRequest: SessionEngine.DeviceScreens?
    /// Les vues de la session, ensemble pour la règle Auto de la souris.
    private let viewGroup = StreamViewGroup()
    /// Surface qui porte la couche SwiftUI : l'écran principal pendant la
    /// connexion, l'écran du geste quand la barre s'ouvre.
    private var chromeSurface: ScreenSurface?
    /// Minuteur d'ouverture de la barre : sa surface.
    private var openBarSurface: ScreenSurface?
    /// Écrans pris par la session (Statistiques), et écrans attendus (étape
    /// « Préparation des écrans »).
    @Published private(set) var screenCount = 1
    @Published private(set) var expectedScreens = 1
    /// La connexion a repris à sa première étape en session (écrans de ce
    /// Mac ou profil changés) : les étapes reviennent sur l'écran principal.
    @Published private(set) var replanning = false

    /// Plusieurs écrans dans cette session (pris ou attendus).
    var multiScreen: Bool { screenCount > 1 || expectedScreens > 1 || !extraSurfaces.isEmpty }
    /// « Plein écran » n'est proposé qu'avec un écran (barre, menu, raccourci).
    var canToggleFullScreen: Bool { !multiScreen }
    /// Écran affiché en ce moment.
    @Published private(set) var shownDisplay: UInt32?

    /// Hauteur de la bande où vit la barre au survol.
    static let barBand: CGFloat = 96
    /// Le pointeur doit rester contre le bord haut de l'image ce temps-là pour
    /// ouvrir la barre : un passage rapide ne l'ouvre pas.
    private static let barOpenDelay: Duration = .milliseconds(250)
    /// La barre se referme ce temps après que le pointeur l'a quittée.
    private static let barCloseDelay: Duration = .milliseconds(400)
    /// « Contre le bord haut » : à 2 points au plus du haut de l'image.
    private static let barEdge: CGFloat = 2
    /// Marge autour de la barre où le pointeur la tient encore ouverte.
    private static let barMargin: CGFloat = 12
    /// Largeur de la barre avant sa première mesure (zone d'ouverture).
    private static let barEstimatedWidth: CGFloat = 1020
    /// Rectangle de la barre dans la vue de session (origine en haut à
    /// gauche), mesuré par SwiftUI ; gardé quand la barre se ferme, pour que
    /// la zone d'ouverture ait sa largeur réelle.
    var barRect: CGRect = .zero
    /// Un menu de la barre est ouvert : elle reste là tant qu'il l'est, puis
    /// se referme si le pointeur est resté loin d'elle.
    var barMenuOpen = false {
        didSet {
            guard barMenuOpen != oldValue else { return }
            if barMenuOpen { cancelBarTimers() } else if let surface = chromeSurface { pointerMoved(barView.pointerSpot, on: surface) }
        }
    }
    /// Bande occupée par la barre, depuis le bord haut de l'image (plus haute
    /// avec le bandeau manette) : la barre mesurée et de quoi voir son ombre ;
    /// avant la première mesure, la bande prévue.
    var barHeight: CGFloat {
        guard !barRect.isEmpty else { return padMode ? 150 : Self.barBand }
        return barRect.maxY + (padMode ? 84 : 30)
    }

    init(host: Host, store: HostStore, opening: Opening, onFinish: @escaping (SessionController) -> Void) {
        self.host = host
        self.store = store
        self.opening = opening
        self.onFinish = onFinish
        guest = nil
        share = ShareHostModel(console: nil) { [store, id = host.id] in ShareHostModel.availability(of: id, in: store) }
    }

    /// Une session d'invité : un invité d'une Station qui a accepté un code (`Share.join`).
    init(guest found: ShareFound, store: HostStore, opening: Opening, onFinish: @escaping (SessionController) -> Void) {
        var host = Host(name: found.computer, address: found.target.host, consolePort: found.target.port, username: "")
        // Pas d'écran virtuel, la souris du bureau, rien de partagé : l'image
        // est celle de l'hôte ; seul le repli du transport vient d'ici.
        host.settings.apply(.jeu)
        host.settings.display = .physical
        host.settings.mouse = .desktop
        host.settings.clipboard = false
        host.settings.keepLastImage = false
        self.host = host
        self.store = store
        self.opening = opening
        self.onFinish = onFinish
        guest = found
        shareRole = found.role
        shareQuality = found.quality
        share = ShareHostModel(console: nil) { .available }
    }

    // MARK: - Fenêtre

    /// Les réglages de l'ordinateur vus par cette session : une session de
    /// plus prend « Cet écran », à la taille de sa fenêtre (sauf sans écran
    /// virtuel), quel que soit le réglage « Écrans ».
    private var settings: ConnectionSettings {
        var s = host.settings
        if !isFirst, s.display != .physical { s.display = .window }
        return s
    }

    /// Les écrans de ce Mac donnés au cœur ; pour une session de plus, sa
    /// règle : « Cet écran », celui de la fenêtre active (`current`).
    private func deviceScreens(current: NSScreen?) -> (screens: [NSScreen], request: SessionEngine.DeviceScreens) {
        let devices = DisplayInfo.deviceScreens(current: current, settings: settings, info: store.status[host.id]?.info)
        guard !isFirst else { return devices }
        return (devices.screens, SessionEngine.screensForExtraSession(devices.request))
    }

    /// La fenêtre de la session. La première prend la place de l'accueil
    /// (avec plusieurs écrans, centrée sur l'écran principal `primary`),
    /// barre de titre masquée comme lui ; les suivantes ont leur barre de
    /// titre, au nom de l'ordinateur, et passent par-dessus une session en
    /// plein écran (`.fullScreenAuxiliary`, échangé contre
    /// `.fullScreenPrimary` le temps de leur propre plein écran).
    private func makeWindow(on primary: NSScreen?, home: NSWindow?) -> NSWindow {
        let window: NSWindow
        switch opening {
        case .first:
            let fallback = NSScreen.main ?? NSScreen.screens[0]
            var frame = home?.frame ?? Self.centered(NSSize(width: 1100, height: 788), on: fallback)
            if let primary, (home?.screen ?? fallback) != primary {
                frame = Self.centered(frame.size, on: primary)
            }
            window = NSWindow(contentRect: frame, styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
                              backing: .buffered, defer: false)
            window.setFrame(frame, display: false)
            window.titlebarAppearsTransparent = true
            window.titleVisibility = .hidden
            window.collectionBehavior = [.fullScreenPrimary, .managed]
        case let .next(anchor):
            // La zone d'image « centrée, aux deux tiers de l'écran, en 16:9 »,
            // règle du cœur ; la barre de titre s'ajoute autour.
            let fallback = Self.centered(NSSize(width: 1100, height: 788), on: anchor?.screen ?? NSScreen.main ?? NSScreen.screens[0])
            let content = extraWindow.map { DisplayInfo.cocoaRect(x: $0.x, y: $0.y, width: $0.width, height: $0.height) } ?? fallback
            window = NSWindow(contentRect: content, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
            window.setFrame(window.frameRect(forContentRect: content), display: false)
            window.appearance = NSAppearance(named: .darkAqua)
            window.collectionBehavior = [.fullScreenAuxiliary, .managed]
        }
        window.title = title
        window.tabbingMode = .disallowed
        window.isRestorable = false
        window.isReleasedWhenClosed = false
        window.backgroundColor = .black
        return window
    }

    /// Un cadre de cette taille (au plus la partie utile de l'écran), centré sur `screen`.
    private static func centered(_ size: NSSize, on screen: NSScreen) -> NSRect {
        let area = screen.visibleFrame
        let w = min(size.width, area.width), h = min(size.height, area.height)
        return NSRect(x: area.midX - w / 2, y: area.midY - h / 2, width: w, height: h)
    }

    /// `primary` : avec plusieurs écrans, la fenêtre s'ouvre sur l'écran
    /// principal de ce Mac (celui de la connexion).
    private func openWindow(on primary: NSScreen? = nil) {
        var home: NSWindow?
        if case let .first(window) = opening, let window, window.isVisible, !window.isMiniaturized, let view = window.contentView {
            home = window
            homeImage = Self.photograph(view)
        }
        let window = makeWindow(on: primary, home: home)
        self.window = window
        let container = NSView(frame: NSRect(origin: .zero, size: window.contentRect(forFrameRect: window.frame).size))
        container.autoresizingMask = [.width, .height]
        streamView.frame = container.bounds
        streamView.autoresizingMask = [.width, .height]
        container.addSubview(streamView)
        scene.frame = container.bounds
        scene.autoresizingMask = [.width, .height]
        container.addSubview(scene)
        scene.prepare(home: homeImage)
        let chrome = ChromeHostingView(rootView: SessionChrome(session: self))
        chrome.session = self
        chrome.frame = container.bounds
        chrome.autoresizingMask = [.width, .height]
        container.addSubview(chrome)
        self.chrome = chrome
        window.contentView = container
        let main = ScreenSurface(main: streamView, container: container, window: window)
        main.installBackdrop()
        // Première connexion : le passage sert de fond ; le fond des écrans ne revient qu'en session.
        main.backdrop.isHidden = true
        mainSurface = main
        chromeSurface = main
        configure(main)
        // Après chaque changement d'état : ce qui n'a rien à montrer sort de la composition.
        compositing = objectWillChange.receive(on: RunLoop.main).sink { [weak self] _ in self?.updateCompositing() }
        main.observe(occlusion: { [weak self] in self?.occlusionChanged($0) },
                     fullScreen: { [weak self] in self?.fullScreenEntered($0) })
        // Écran branché, débranché, définition changée ; fenêtre passée sur un autre écran.
        screenObservers = [
            NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.screensChanged() }
            },
            NotificationCenter.default.addObserver(forName: NSWindow.didChangeScreenNotification, object: window, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.screensChanged() }
            },
        ]
        appActive = NSApp.isActive
        activeObservers = [NSApplication.didBecomeActiveNotification, NSApplication.didResignActiveNotification].map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.appActive = NSApp.isActive }
            }
        }
        // Fermer la fenêtre (bouton rouge, ⌘W), c'est se déconnecter : elle
        // se ferme d'elle-même une fois la session finie.
        windowDelegate.onClose = { [weak self] in self?.closeRequested() }
        windowDelegate.onExitFullScreen = { [weak self] in self?.fullScreenExited() }
        window.delegate = windowDelegate
        window.acceptsMouseMovedEvents = true
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(streamView)
        scene.ignite()
        // La première session passe en plein écran, sauf fenêtrée ; les
        // suivantes restent dans leur fenêtre (« Plein écran » dans la barre).
        // Démonstration : la fenêtre reste telle quelle, pour la capturer
        // (`-demoFullScreen YES` : plein écran comme en vrai).
        if isFirst, settings.display != .window, !window.styleMask.contains(.fullScreen),
           !AppModel.demo || UserDefaults.standard.bool(forKey: "demoFullScreen") {
            window.toggleFullScreen(nil)
        }
    }

    /// Bouton rouge, ⌘W : la session se termine (un échec affiché se ferme).
    private func closeRequested() {
        guard phase != .ended else { return }
        closedByWindow = true
        if isLive { end() } else { dismissFailure() }
    }

    /// Pendant le flux, seule la couche Metal doit rester dans la composition
    /// de la fenêtre pour que l'image aille directement à l'écran : la couche
    /// SwiftUI est cachée tant qu'elle n'a rien à montrer (barre, manette,
    /// rappel, panneau des mesures, fin), et le passage une fois
    /// ouvert (son animation dure 1,1 s). Hors flux, tout revient.
    private func updateCompositing() {
        let streaming = phase == .streaming
        let needed = !streaming || barVisible || padMode || hint != nil || statsPanel || screenOfferShown
        if chrome?.isHidden == needed { chrome?.isHidden = !needed }
        // Trait de repère de la barre, tracé dans l'image : là où le geste qui
        // ouvre la barre est possible (app au premier plan, session active,
        // souris du bureau, barre fermée), pas ailleurs.
        let marker = streaming && !barVisible && appActive && focusActive && !mouseCaptured
        if marker != markerShown {
            markerShown = marker
            engine?.setTopMarker(marker)
        }
        if streaming {
            guard sceneHide == nil, !scene.isHidden else { return }
            sceneHide = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(1400))
                guard let self, !Task.isCancelled, self.phase == .streaming else { return }
                self.scene.isHidden = true
            }
        } else {
            sceneHide?.cancel()
            sceneHide = nil
            if scene.isHidden { scene.isHidden = false }
        }
    }

    /// Fenêtre plus visible (autre espace, réduite dans le Dock, recouverte) :
    /// plus de rendu (`nextDrawable` y attendrait jusqu'à 1 s par image) et débit plafonné
    /// à 20 Mb/s ; au retour, rendu et débit du profil reviennent.
    private func occlusionChanged(_ surface: ScreenSurface) {
        guard window != nil else { return }
        // Plusieurs écrans : chaque fenêtre arrête et reprend son propre rendu
        // (une fenêtre qui passe en plein écran est vue occultée un instant) ;
        // à la reprise, son image est redessinée et une image clé demandée.
        if multiScreen, let index = deviceIndex(of: surface) {
            let hidden = surface.occluded
            engine?.setScreenOccluded(hidden, screen: index)
            KyberLog.app.notice("fenêtre de l'écran \(index) \(hidden ? "occultée : son rendu attend" : "visible : son rendu reprend", privacy: .public)")
            if !hidden { engine?.refreshScreen(index) }
        }
        // Débit réduit et veille d'image en attente seulement quand aucune fenêtre ne se voit.
        let hidden = surfaces.allSatisfy(\.occluded)
        guard hidden != occluded else { return }
        occluded = hidden
        engine?.setOccluded(hidden)
        engine?.setBitrateCap(hidden ? Self.occludedBitrate : nil)
        let which = multiScreen ? "toutes les fenêtres" : "fenêtre"
        KyberLog.app.notice("\(which, privacy: .public) \(hidden ? "occultée(s) : rendu arrêté, débit réduit" : "visible(s) : rendu et débit repris", privacy: .public)")
    }

    /// Multi-écrans : une fenêtre a fini de passer en plein écran : son image
    /// est redessinée à sa taille et une image clé est demandée (une image
    /// présentée pendant l'animation a pu être écartée par l'écran).
    private func fullScreenEntered(_ surface: ScreenSurface) {
        guard multiScreen, let index = deviceIndex(of: surface) else { return }
        KyberLog.app.notice("fenêtre de l'écran \(index) en plein écran")
        engine?.refreshScreen(index)
    }

    /// L'écran de ce Mac (index du cœur) que montre cette surface.
    private func deviceIndex(of surface: ScreenSurface) -> Int? {
        deviceScreens.indices.first { self.surface(forDevice: $0) === surface }
    }

    /// Photographie de l'accueil, pour les deux moitiés du passage.
    private static func photograph(_ view: NSView) -> CGImage? {
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return nil }
        view.cacheDisplay(in: view.bounds, to: rep)
        return rep.cgImage
    }

    func toggleFullScreen() {
        guard canToggleFullScreen, let window else { return }
        // Une session de plus : sa fenêtre passe par-dessus le plein écran
        // d'une autre (auxiliaire) ; pour le sien, elle prend son propre espace.
        if !window.styleMask.contains(.fullScreen), window.collectionBehavior.contains(.fullScreenAuxiliary) {
            window.collectionBehavior = [.fullScreenPrimary, .managed]
        }
        window.toggleFullScreen(nil)
    }

    /// Sortie du plein écran : une session de plus redevient une fenêtre qui
    /// peut passer par-dessus le plein écran d'une autre.
    private func fullScreenExited() {
        guard !isFirst, phase != .ended, let window, !window.styleMask.contains(.fullScreen) else { return }
        window.collectionBehavior = [.fullScreenAuxiliary, .managed]
    }

    /// La fenêtre de la session est en plein écran.
    var isFullScreen: Bool { window?.styleMask.contains(.fullScreen) == true }

    /// Les fenêtres de la session (celle de la session, puis celles des autres écrans).
    var windows: [NSWindow] { surfaces.compactMap(\.window) }

    /// Cette fenêtre est-elle à la session ?
    func owns(_ window: NSWindow) -> Bool {
        windows.contains { $0 === window }
    }

    /// « Afficher », « Ordinateur suivant », « Se connecter » sur un
    /// ordinateur déjà en session : sa fenêtre passe devant (son espace, en
    /// plein écran ; celles de ses autres écrans aussi).
    func bringToFront() {
        guard let window else { return }
        Dock.comeForward()
        for extra in extraSurfaces { extra.window?.orderFront(nil) }
        if window.isMiniaturized { window.deminiaturize(nil) }
        window.makeKeyAndOrderFront(nil)
    }

    /// L'identifiant de la session dans le registre du cœur (nil avant son
    /// départ, hors registre ou une fois finie).
    var focusID: Int64? { engine?.focusID }

    /// La fenêtre de la session passe au premier plan : elle devient active
    /// dans le registre du cœur, qui coupe les entrées et le son des autres.
    func focusActivate() {
        engine?.focusActivate()
    }

    /// « Garder le son », par le registre du cœur (démonstration, sans cœur :
    /// l'interface seule).
    func setKeepSound(_ on: Bool) {
        guard let engine, engine.focusID != nil else {
            keepSound = on
            return
        }
        engine.keepSound = on
        KyberLog.app.notice("\(self.host.name, privacy: .public) : garder le son \(on ? "allumé" : "éteint", privacy: .public)")
    }

    /// L'état du registre, pour l'interface (Sessions.swift) : une session
    /// inactive n'allume pas le trait de repère, ferme sa barre, ignore la
    /// combinaison de la manette et ne suit plus le pointeur dans ses
    /// fenêtres d'autres écrans. Les entrées, le son, le presse-papiers et
    /// les manettes, le cœur s'en est déjà chargé.
    func focusChanged(active: Bool) {
        if let engine, engine.focusID != nil, keepSound != engine.keepSound { keepSound = engine.keepSound }
        guard focusActive != active else { return }
        focusActive = active
        KyberLog.app.notice("\(self.host.name, privacy: .public) : session \(active ? "active" : "inactive", privacy: .public)")
        let multi = multiScreen
        allViews.forEach { $0.tracksInActiveApp = multi && active }
        if !active { setBar(false) }
    }

    /// Les couches SwiftUI captent-elles tout (attente, fin) ?
    var chromeCapturesAll: Bool {
        phase != .streaming
    }

    // MARK: - Écrans de ce Mac (multi-écrans)

    /// Toutes les surfaces de la session, celle de sa fenêtre d'abord.
    private var surfaces: [ScreenSurface] { [mainSurface].compactMap { $0 } + extraSurfaces }

    /// Toutes les vues de session.
    private var allViews: [StreamView] { surfaces.map(\.streamView) }

    /// Réglages d'une vue de session (la même pour chaque écran).
    private func configure(_ surface: ScreenSurface) {
        let view = surface.streamView
        let settings = host.settings
        view.commandAsControl = settings.commandAsControl
        view.mouseMode = settings.mouse.streamMode
        view.pcGameFullscreen = pcGame
        view.tracksInActiveApp = multiScreen && focusActive
        view.onPointerMoved = { [weak self, weak surface] spot in
            guard let self, let surface else { return }
            self.pointerMoved(spot, on: surface)
        }
        view.onRelativeChanged = { [weak self] on in
            guard let self else { return }
            self.mouseCaptured = self.allViews.contains(where: \.isRelative)
            // Premier passage en mode jeu : on dit comment reprendre la souris.
            guard on, !self.mouseHintShown else { return }
            self.mouseHintShown = true
            self.showHint(.mouse, after: 0)
        }
        viewGroup.add(view)
    }

    /// Une fenêtre de plus, plein écran sur un autre écran de ce Mac.
    @discardableResult
    private func addSurface(for screen: NSScreen) -> ScreenSurface {
        let surface = ScreenSurface(screen: screen, title: title) { [weak self] in
            guard let self, self.phase != .ended else { return }
            self.end()
        }
        surface.observe(occlusion: { [weak self] in self?.occlusionChanged($0) },
                        fullScreen: { [weak self] in self?.fullScreenEntered($0) })
        extraSurfaces.append(surface)
        // La fenêtre de la session suit aussi les mouvements d'un écran à l'autre (session active).
        let active = focusActive
        allViews.forEach { $0.tracksInActiveApp = active }
        configure(surface)
        KyberLog.app.notice("fenêtre ouverte sur l'écran \(surface.screenID ?? 0)")
        return surface
    }

    private func closeSurface(_ surface: ScreenSurface) {
        if chromeSurface === surface, let main = mainSurface { moveChrome(to: main) }
        if openBarSurface === surface { cancelBarTimers() }
        viewGroup.remove(surface.streamView)
        surface.streamView.detach()
        surface.close()
        extraSurfaces.removeAll { $0 === surface }
        KyberLog.app.notice("fenêtre fermée sur l'écran \(surface.screenID ?? 0)")
    }

    /// La surface d'un écran de ce Mac (fenêtre de la session si elle y est).
    private func surface(on screen: NSScreen) -> ScreenSurface? {
        guard let id = DisplayInfo.displayID(screen) else { return nil }
        if let main = mainSurface, let w = window?.screen, DisplayInfo.displayID(w) == id { return main }
        return extraSurfaces.first { $0.screenID == id }
    }

    /// La surface de l'écran `index` de la liste donnée au cœur.
    private func surface(forDevice index: Int) -> ScreenSurface? {
        guard index >= 0, index < deviceScreens.count else { return nil }
        return surface(on: deviceScreens[index])
    }

    /// Le cœur sait quelle vue montre chaque écran de ce Mac.
    private func syncEngineViews() {
        var map: [Int: StreamView] = [:]
        for i in deviceScreens.indices {
            if let s = surface(forDevice: i) { map[i] = s.streamView }
        }
        engine?.setScreenViews(map)
    }

    /// Les écrans pris par la session (avant leurs images) : les fenêtres
    /// manquantes s'ouvrent, celles que la session ne prend plus se ferment
    /// (Station à un écran, principal seul accepté, session repassée à un
    /// écran), un écran que l'ordinateur n'a pas pu créer garde le fond avec
    /// sa ligne (docs/link-ui.md).
    private func screensTaken(_ streams: [SessionEngine.StreamScreen], _ notCreated: [Int]) {
        // Un seul flux, sans écran non créé : une session à un écran (ou le
        // principal seul accepté). Jamais de fenêtre de plus : l'image va dans
        // la fenêtre de la session, quel que soit l'écran que le cœur nomme.
        if streams.count == 1, notCreated.isEmpty, let only = streams.first, let main = mainSurface {
            extraSurfaces.forEach(closeSurface)
            engine?.setScreenViews([only.index: main.streamView])
            main.backdrop.message = nil
            screenCount = 1
            expectedScreens = 1
            main.streamView.tracksInActiveApp = false
            if phase != .streaming || replanning { moveChrome(to: main) }
            return
        }
        let wanted = Set(streams.map(\.index) + notCreated)
        for i in wanted.sorted() where i < deviceScreens.count && surface(forDevice: i) == nil {
            addSurface(for: deviceScreens[i]).backdrop.isHidden = false
        }
        for extra in extraSurfaces where !wanted.contains(where: { surface(forDevice: $0) === extra }) {
            closeSurface(extra)
        }
        syncEngineViews()
        for i in notCreated {
            guard let s = surface(forDevice: i) else { continue }
            s.backdrop.message = LinkText.screenNotCreated
            s.backdrop.isHidden = false
        }
        for st in streams { surface(forDevice: st.index)?.backdrop.message = nil }
        screenCount = max(1, streams.count)
        expectedScreens = max(1, streams.count + notCreated.count)
        let multi = multiScreen && focusActive
        allViews.forEach { $0.tracksInActiveApp = multi }
        // La connexion s'affiche sur l'écran principal.
        if phase != .streaming || replanning, let p = streams.first(where: \.primary), let s = surface(forDevice: p.index) {
            moveChrome(to: s)
        }
    }

    /// La couche SwiftUI (connexion, barre, rappels, mesures) passe sur cette surface.
    private func moveChrome(to surface: ScreenSurface) {
        guard let chrome, chrome.superview !== surface.container else {
            chromeSurface = surface
            return
        }
        chrome.removeFromSuperview()
        chrome.frame = surface.container.bounds
        surface.container.addSubview(chrome, positioned: .above, relativeTo: nil)
        chromeSurface = surface
    }

    /// La connexion reprend en session : chaque écran retrouve le fond de la
    /// connexion jusqu'à sa première image.
    private func showBackdrops() {
        for s in surfaces {
            s.backdrop.message = nil
            s.backdrop.isHidden = false
        }
    }

    /// Les écrans de ce Mac donnés au cœur, à jour ; les fenêtres d'écrans
    /// débranchés se ferment tout de suite. Rend la nouvelle description si
    /// elle a changé.
    private func refreshDeviceScreens() -> SessionEngine.DeviceScreens? {
        let devices = deviceScreens(current: window?.screen)
        let ids = Set(devices.screens.compactMap(DisplayInfo.displayID))
        for extra in extraSurfaces where !(extra.screenID.map(ids.contains) ?? false) { closeSurface(extra) }
        deviceScreens = devices.screens
        syncEngineViews()
        guard devices.request != screensRequest else { return nil }
        screensRequest = devices.request
        return devices.request
    }

    /// Écrans attendus d'après le plan du cœur (étape « Préparation des écrans »).
    private func updateExpectedScreens() {
        guard let request = screensRequest else { return }
        let plan = SessionEngine.sessionPlan(request, profile: settings.profile?.rawValue,
                                             virtualDisplay: virtualDisplay, codec: settings.codec)
        expectedScreens = plan?.count ?? 1
    }

    // MARK: - Barre au survol

    /// La vue où vit la barre (celle de la surface qui porte la couche SwiftUI).
    private var barView: StreamView { chromeSurface?.streamView ?? streamView }

    /// Largeur de la zone d'ouverture : celle de la barre, mesurée ou estimée.
    private var barWidth: CGFloat { barRect.width > 0 ? barRect.width : Self.barEstimatedWidth }

    /// Contre le bord haut de l'image, dans la largeur de la barre, centrée.
    private func inOpenZone(_ spot: CGPoint, of view: StreamView) -> Bool {
        spot.y <= Self.barEdge && abs(spot.x - view.bounds.width / 2) <= barWidth / 2
    }

    /// Sur la barre, à 12 points près, ou entre elle et le bord haut de
    /// l'image (là où le pointeur l'a ouverte).
    private func keepsBarOpen(_ spot: CGPoint) -> Bool {
        let rect = barRect.isEmpty
            ? CGRect(x: (barView.bounds.width - barWidth) / 2, y: 0, width: barWidth, height: Self.barBand)
            : barRect
        return spot.x >= rect.minX - Self.barMargin && spot.x <= rect.maxX + Self.barMargin && spot.y <= rect.maxY + Self.barMargin
    }

    private func cancelBarTimers() {
        openBar?.cancel()
        openBar = nil
        openBarSurface = nil
        hideBar?.cancel()
        hideBar = nil
    }

    /// Le pointeur a bougé sur `surface` (`spot` : sa place sur l'image de
    /// cette surface, nil s'il n'y est plus). Barre fermée, le rester contre
    /// le bord haut, dans la largeur de la barre, la fait venir sur cet écran,
    /// sauf un bouton de la souris tenu (une fenêtre de l'ordinateur glissée
    /// contre le bord) ; barre ouverte, la quitter la fait partir. Dans les
    /// deux cas par un minuteur : un pointeur immobile n'envoie plus rien, et
    /// il a fini de passer ou de partir quand le minuteur sonne.
    private func pointerMoved(_ spot: CGPoint?, on surface: ScreenSurface) {
        guard phase == .streaming else { return }
        if barVisible {
            openBar?.cancel()
            openBar = nil
            openBarSurface = nil
            // La manette ou un menu tient la barre ouverte.
            guard !padMode, !barMenuOpen else {
                hideBar?.cancel()
                hideBar = nil
                return
            }
            // Un autre écran : le pointeur a quitté la barre.
            let onBar = surface === chromeSurface ? spot : nil
            if let onBar, keepsBarOpen(onBar) {
                hideBar?.cancel()
                hideBar = nil
            } else if hideBar == nil {
                hideBar = Task { [weak self] in
                    try? await Task.sleep(for: Self.barCloseDelay)
                    guard !Task.isCancelled else { return }
                    self?.closeBarIfAway()
                }
            }
        } else if let spot, inOpenZone(spot, of: surface.streamView), NSEvent.pressedMouseButtons == 0 {
            if openBar == nil || openBarSurface !== surface {
                openBar?.cancel()
                openBarSurface = surface
                openBar = Task { [weak self, weak surface] in
                    try? await Task.sleep(for: Self.barOpenDelay)
                    guard !Task.isCancelled, let surface else { return }
                    self?.openBarIfAtEdge(on: surface)
                }
            }
        } else if openBarSurface == nil || openBarSurface === surface {
            openBar?.cancel()
            openBar = nil
            openBarSurface = nil
        }
    }

    /// Une fenêtre de la session a le clavier (avec plusieurs écrans,
    /// n'importe laquelle : la barre s'ouvre là où est le pointeur).
    private func sessionWindowIsKey(_ surface: ScreenSurface) -> Bool {
        guard multiScreen else { return window?.isKeyWindow == true }
        return surfaces.contains { $0.window?.isKeyWindow == true }
    }

    /// Le pointeur est resté contre le bord : la barre s'ouvre si tout y est
    /// encore (app au premier plan, fenêtre de la session clé, pointeur sur
    /// l'image, souris en mode bureau, aucun bouton tenu), sur cet écran.
    private func openBarIfAtEdge(on surface: ScreenSurface) {
        openBar = nil
        openBarSurface = nil
        guard phase == .streaming, !barVisible, NSApp.isActive, sessionWindowIsKey(surface), !surface.streamView.isRelative,
              NSEvent.pressedMouseButtons == 0, let spot = surface.streamView.pointerSpot, inOpenZone(spot, of: surface.streamView) else { return }
        if surface !== chromeSurface {
            moveChrome(to: surface)
            surface.window?.makeKey()
        }
        setBar(true)
    }

    /// Le pointeur a quitté la barre : elle se ferme, sauf s'il y est revenu
    /// (aucun mouvement n'arrive tant qu'il est sur ses boutons : on regarde où il est).
    private func closeBarIfAway() {
        hideBar = nil
        guard phase == .streaming, barVisible, !padMode, !barMenuOpen else { return }
        if let spot = barView.pointerSpot, keepsBarOpen(spot) { return }
        setBar(false)
    }

    func setBar(_ visible: Bool) {
        cancelBarTimers()
        if !visible, padMode {
            padMode = false
            engine?.setGamepadCaptured(false)
        }
        guard visible != barVisible else { return }
        withAnimation(.spring(response: 0.32, dampingFraction: 0.86)) { barVisible = visible }
        for view in allViews {
            view.reservedTopBand = visible && view === barView ? 150 : 0
            view.relativeSuspended = visible
            view.window?.invalidateCursorRects(for: view)
        }
    }

    // MARK: - Manette

    /// L1 + R1 + START + SELECT : la barre s'ouvre et la manette la pilote,
    /// ou se ferme et la manette revient à l'ordinateur. Hors session
    /// (connexion, reconnexion), rien : la manette reste à l'ordinateur.
    private func gamepadCombo() {
        // Seule la session active a la manette (plusieurs ordinateurs).
        guard phase == .streaming, focusActive else { return }
        if padMode {
            setBar(false)
        } else {
            padFocus = 0
            withAnimation(.easeOut(duration: 0.2)) { padMode = true }
            engine?.setGamepadCaptured(true)
            setBar(true)
        }
    }

    private func gamepadNavigate(_ key: PadKey) {
        guard padMode else { return }
        switch key {
        case .left: padFocus = (padFocus + padItems - 1) % padItems
        case .right: padFocus = (padFocus + 1) % padItems
        case .back: setBar(false)
        case .accept: activatePadFocus()
        case .up, .down: break
        }
    }

    private func activatePadFocus() {
        if isGuest {
            // Qualité de l'image (suivante : Auto → Comme l'hôte → 1080p → 720p), plein écran, quitter.
            if padFocus == 0 {
                let all = ShareQuality.allCases
                setShareQuality(all[((all.firstIndex(of: shareQuality) ?? 0) + 1) % all.count])
            } else if padFocus == 1, canToggleFullScreen {
                toggleFullScreen()
            } else {
                end()
            }
            return
        }
        switch padFocus {
        case 0:
            // Profil suivant (Jeu → Bureau → Max).
            let all = Profile.allCases
            let next = host.settings.profile.flatMap { all.firstIndex(of: $0) }.map { all[($0 + 1) % all.count] } ?? .jeu
            select(next)
        case 1:
            // Souris suivante (Auto → Bureau → Jeu).
            let all = MouseChoice.allCases
            let i = all.firstIndex(of: host.settings.mouse) ?? 0
            setMouse(all[(i + 1) % all.count])
        case 2 where canToggleFullScreen:
            toggleFullScreen()
        default:
            end()
        }
    }

    /// Change de profil en cours de session : le cœur change le débit en
    /// direct, ou relance l'image si le 4:4:4, le format ou le transport
    /// changent (une seconde environ) ; le repli de la session repart de zéro.
    func select(_ profile: Profile) {
        // Un invité ne change rien chez l'hôte : ni profil, ni réglages.
        guard !isGuest, profile != host.settings.profile else { return }
        commit { $0.settings.apply(profile) }
        // Multi-écrans : le nombre d'écrans suit le profil tout de suite (le
        // cœur refait la session s'il change).
        updateExpectedScreens()
        engine?.setSettings(engineSettings)
        engine?.setVSync(host.settings.vsync)
    }

    /// Les réglages de l'image, pour le cœur (il calcule le débit du mode).
    private var engineSettings: SessionEngine.Settings {
        let s = host.settings
        return SessionEngine.Settings(
            profile: s.profile?.rawValue,
            codec: s.codec,
            yuv444: s.yuv444 && ConnectionSettings.decodes444(s.codec),
            mbps: s.bitrateMbps,
            reliable: s.transport == .reliable
        )
    }

    // MARK: - Déroulé

    func start() {
        let settings = self.settings
        // L'écran où s'ouvre la fenêtre : celui de l'accueil, ou de la fenêtre active.
        let current: NSScreen
        switch opening {
        case let .first(home): current = home?.screen ?? NSScreen.main ?? NSScreen.screens[0]
        case let .next(anchor): current = anchor?.screen ?? NSScreen.main ?? NSScreen.screens[0]
        }
        // Multi-écrans : le cœur décide du nombre d'écrans (réglage, profil,
        // Station) ; plusieurs, la fenêtre passe sur l'écran principal, et
        // chaque autre écran pris a sa fenêtre.
        let devices = deviceScreens(current: current)
        var request = devices.request
        // Une session de plus : sa fenêtre, cadre et mode, règle du cœur.
        if !isFirst { extraWindow = SessionEngine.extraSessionWindow(request) }
        // Un invité n'a qu'un écran : celui de l'hôte, sans écran virtuel ni plan.
        let plan = isGuest ? nil : SessionEngine.sessionPlan(request, profile: settings.profile?.rawValue,
                                                             virtualDisplay: settings.display != .physical, codec: settings.codec)
        let primary = plan != nil ? devices.screens.first : nil
        deviceScreens = devices.screens
        expectedScreens = plan?.count ?? 1
        openWindow(on: primary)
        // « Cet écran » : celui où est maintenant la fenêtre (l'écran
        // principal, sauf une fenêtre déjà en plein écran ailleurs).
        let mainID = (window?.screen).flatMap(DisplayInfo.displayID)
        request.current = deviceScreens.firstIndex { DisplayInfo.displayID($0) == mainID } ?? request.current
        screensRequest = request
        if let plan {
            for p in plan where surface(forDevice: p.device) == nil {
                addSurface(for: deviceScreens[p.device])
            }
        }
        // Un invité n'a pas de compte : le code a suffi.
        let password = isGuest ? "" : store.password(for: host)
        guard let password else {
            fail(String(localized: "mot de passe introuvable dans le Trousseau"))
            return
        }
        let screen = window?.screen ?? primary ?? current
        let mode: (width: Int, height: Int, refresh: Int)
        switch settings.display {
        case .custom:
            mode = (settings.customWidth, settings.customHeight, settings.customRefresh)
        case .window where extraWindow != nil:
            // Session de plus : le mode de sa zone d'image, calculé par le cœur.
            let m = extraWindow?.mode ?? SessionEngine.Mode(width: 0, height: 0, refresh: 0)
            mode = (m.width, m.height, m.refresh)
        case .window:
            // Session fenêtrée : l'écran virtuel prend la taille de la fenêtre, en pixels.
            // Pixels de la dalle par point (moins que l'échelle de macOS en mode « plus d'espace »).
            let content = window?.contentLayoutRect.size ?? screen.frame.size
            let scale = CGFloat(DisplayInfo.nativePixels(screen).width) / screen.frame.width
            mode = (Int(content.width * scale) / 2 * 2, Int(content.height * scale) / 2 * 2, max(30, screen.maximumFramesPerSecond))
        case .match, .physical:
            mode = DisplayInfo.mode(for: screen, codec: settings.codec, limit4K: settings.limit4K)
        }
        modeLabel = "\(mode.width) × \(mode.height) · \(mode.refresh) Hz"
        streamMode = mode
        requested = ScreenMode(width: mode.width, height: mode.height, refresh: mode.refresh)
        virtualDisplay = settings.display != .physical

        let engine = SessionEngine(view: streamView)
        engine.onPhase = { [weak self] in self?.phaseChanged($0) }
        engine.onNotice = { [weak self] in self?.notice($0) }
        engine.onDisplays = { [weak self] in self?.screens = $0 }
        engine.onShownDisplay = { [weak self] in self?.shownDisplay = $0.id }
        engine.onMode = { [weak self] mode, virtualDisplay in
            guard let self else { return }
            self.streamMode = (mode.width, mode.height, mode.refresh)
            self.modeLabel = "\(mode.width) × \(mode.height) · \(mode.refresh) Hz"
            self.virtualDisplay = virtualDisplay
        }
        engine.onVideoStats = { [weak self] stats in
            guard let self else { return }
            self.stats = stats
            if let ms = stats.latencyMs {
                self.latencyHistory.append(ms)
                if self.latencyHistory.count > 40 { self.latencyHistory.removeFirst() }
            }
            self.statsHistory.append(stats)
            if self.statsHistory.count > 30 { self.statsHistory.removeFirst() }
            self.sessionLosses.frames += stats.lostFrames
            self.sessionLosses.packets += stats.lostPackets
        }
        engine.onAudioStats = { [weak self] in self?.audioStats = $0 }
        engine.onGamepadCombo = { [weak self] in self?.gamepadCombo() }
        engine.onGamepadNavigate = { [weak self] in self?.gamepadNavigate($0) }
        engine.onScreens = { [weak self] in self?.screensTaken($0, $1) }
        engine.onScreenImage = { [weak self] index in
            guard let self else { return }
            // Une seule fenêtre : celle de la session, quel que soit l'écran nommé.
            (self.extraSurfaces.isEmpty ? self.mainSurface : self.surface(forDevice: index))?.backdrop.isHidden = true
        }
        engine.onGameFullscreen = { [weak self] game in
            guard let self, game != self.pcGame else { return }
            KyberLog.app.notice("ordinateur : \(game ? "jeu en plein écran" : "pas de jeu en plein écran", privacy: .public)")
            self.pcGame = game
        }
        // KyberNow Share : la liste des invités (hôte), le rôle et la fin (invité).
        engine.onGuests = { [weak self] in self?.share.heartbeat($0) }
        engine.onShareRole = { [weak self] role, previous in self?.shareRoleChanged(role, previous: previous) }
        engine.onShareEnd = { [weak self] _, text in self?.shareEnded(text) }
        engine.onShareImage = { [weak self] image, _, notice in self?.shareImageChanged(image, notice: notice) }
        self.engine = engine
        syncEngineViews()
        if !isGuest { share.setConsole(store.console(for: host)) }

        var console = host.address
        #if DEBUG
        // Démonstration avec `-demoCore YES` : une vraie session du cœur (pour
        // le registre des sessions), vers une adresse de documentation que rien
        // ne route (RFC 5737) : jamais une Station, elle échoue au bout de 5 s.
        if AppModel.demo { console = "192.0.2.1" }
        #endif
        var params: SessionEngine.Params
        if let guest {
            // Un invité : la Station qui a accepté le code, sans compte ni écran virtuel.
            params = SessionEngine.Params(guest: guest, audioChannels: settings.audio.channels, settings: engineSettings)
        } else {
            params = SessionEngine.Params(
                host: console, consolePort: host.consolePort, username: host.username, password: password,
                consoleFingerprint: host.consoleFingerprint,
                mode: SessionEngine.Mode(width: mode.width, height: mode.height, refresh: mode.refresh),
                audioChannels: settings.audio.channels, virtualDisplay: virtualDisplay, settings: engineSettings
            )
            params.clipboard = settings.clipboard
            params.screens = request
        }
        params.autoReconnect = settings.autoReconnect
        params.gamepadAsXbox360 = settings.gamepad == .xbox360
        params.vsync = settings.vsync
        // Inscrite au registre des sessions du cœur dès son départ (plusieurs ordinateurs).
        params.focusName = host.name
        KyberLog.app.notice("session : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, son \(settings.audio.channels) canaux, \(settings.consoleProfile, privacy: .public)")
        do {
            try engine.start(params)
        } catch SessionEngine.StartError.tooManyComputers {
            fail(LinkText.computersLimit)
        } catch {
            fail(error.localizedDescription)
        }
    }

    /// Les états du cœur : l'image n'apparaît qu'après `revealDelay`, les
    /// fins voulues et les fins en erreur se séparent ici.
    private func phaseChanged(_ p: SessionEngine.Phase) {
        if case .failed = phase {
            // Échec dit par l'app (registre plein…) : la session du cœur finit
            // quand même ; plus rien à attendre d'elle (fin de l'app).
            if case .ended = p, !coreEnded {
                coreEnded = true
                onEnded?(self)
            }
            return
        }
        engineStreaming = p == .streaming
        if p != .connecting { stationRestarting = false }
        switch p {
        case .preparing:
            // En session : la connexion reprend à sa première étape (écrans
            // de ce Mac ou profil changés), sur l'écran principal ; chaque
            // écran retrouve le fond de la connexion jusqu'à sa première image.
            if revealedAt != nil, phase != .preparing {
                setBar(false)
                replanning = true
                showBackdrops()
                if let primary = surface(forDevice: 0) ?? mainSurface { moveChrome(to: primary) }
            }
            phase = .preparing
        case .connecting:
            phase = .connecting
        case .starting:
            if revealedAt == nil || replanning { phase = .starting }
        case .streaming:
            guard revealedAt == nil else {
                replanning = false
                phase = .streaming
                return
            }
            phase = .starting
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.revealDelay) { [weak self] in
                guard let self, self.revealedAt == nil, self.engineStreaming, self.engine != nil else { return }
                // Dès la première image, l'image seule : ni capsule, ni barre, ni chiffre.
                self.revealedAt = Date()
                self.phase = .streaming
                self.scene.open()
                self.window?.makeFirstResponder(self.streamView)
                // L'écran de ce Mac a pu changer pendant la connexion.
                self.checkScreenMode()
            }
        case let .reconnecting(attempt, _):
            phase = .reconnecting(attempt)
        case .ending:
            guard phase != .ending, phase != .ended else { break }
            endedByComputer = true
            // Avant la première image, sans « Se déconnecter » demandé : la
            // connexion a échoué (mot de passe, refus, certificat, Kyber…).
            // La capsule de la connexion reste ; `ended` en dit la raison
            // (« La connexion a échoué »), comme sur Windows et Linux.
            if revealedAt == nil { break }
            // Un invité dont l'hôte a arrêté ou retiré le partage : la raison
            // reste à l'écran (jamais « Reprendre la main »), pas de passage qui se referme.
            if isGuest, let reason = shareEndText {
                fail(reason)
                return
            }
            // Fin demandée par la console (ou connexion perdue sans reprise) :
            // le passage se referme comme pour une fin voulue.
            end()
        case let .ended(error, report):
            if phase != .ending, phase != .ended { endedByComputer = true }
            endError = error
            defer {
                coreEnded = true
                onEnded?(self)
            }
            if let error, phase != .ending, phase != .ended {
                fail(error)
                return
            }
            if phase != .ending, phase != .ended { end() }
            closing.displayRestored = report?.displayRestored ?? false
            closing.audioRestored = report?.audioRestored ?? false
            closing.error = error ?? report?.error
            closing.done = true
            // Le bilan d'Admin seulement : sans lui (Admin injoignable, session
            // jamais préparée), rien à dire des écrans ni du son sur l'accueil.
            if report != nil { onReport?(host, closing) }
        }
    }

    // MARK: - KyberNow Share

    /// L'hôte a changé les droits de l'invité (« Vous pouvez jouer à la
    /// manette. ») ; au départ (`previous` nil), le rôle seulement.
    private func shareRoleChanged(_ role: ShareRole, previous: ShareRole?) {
        // Une session finie, ou qui se ferme, n'affiche plus rien.
        guard isLive, phase != .ending else { return }
        shareRole = role
        // Sans le clavier et la souris, rien ne part vers l'ordinateur : le
        // pointeur du Mac n'est plus capturé (souris des jeux) ; avec, le choix
        // de la barre reprend.
        streamView.mouseMode = role.keyboardAndMouse ? host.settings.mouse.streamMode : .desktop
        if let text = ShareText.roleNotice(role, previous: previous) {
            showHint(.notice(text), after: 0, for: 4)
        }
    }

    /// Fin par le partage : « {hôte} a arrêté le partage. », « {hôte} vous a
    /// retiré du partage. » ; le texte vient du cœur. Dit avant `.ending`.
    private func shareEnded(_ text: String) {
        shareEndText = text
    }

    /// « Qualité de l'image » dans la barre : le cœur relance l'image de
    /// l'invité seulement (jamais celle de l'hôte) ; la qualité est retenue
    /// pour la fois suivante. La qualité reçue est inconnue jusqu'à
    /// `on_share_image` : la barre dit d'ici là la qualité choisie.
    func setShareQuality(_ quality: ShareQuality) {
        guard isGuest, isLive, phase != .ending, quality != shareQuality else { return }
        shareQuality = quality
        shareImage = nil
        GuestQuality.remember(quality)
        engine?.setShareQuality(quality)
    }

    /// Le format de l'image reçue : celui de son image (un invité), à défaut
    /// celui de l'hôte, sinon celui des réglages.
    var imageCodec: VideoCodec { shareImage?.codec ?? guest?.codec ?? host.settings.codec }

    /// L'image que reçoit l'invité a changé (au départ, ou après un changement
    /// de qualité, un retard, une limite de l'ordinateur) ; l'avis du cœur
    /// (« Image allégée pour suivre. ») s'affiche en capsule comme les autres.
    private func shareImageChanged(_ image: ShareImage, notice: String?) {
        guard isGuest, isLive, phase != .ending else { return }
        shareImage = image
        if let notice, !notice.isEmpty { showHint(.notice(notice), after: 0, for: 4) }
    }

    /// Les avis du cœur, en capsules.
    private func notice(_ n: SessionEngine.Notice) {
        switch n {
        case .relaunched:
            showHint(.notice(String(localized: "L'image a été relancée.")), after: 0, for: 4)
        case let .modeChanged(width, height, back):
            streamMode = (width, height, streamMode.refresh)
            modeLabel = "\(width) × \(height) · \(streamMode.refresh) Hz"
            var text = back ? String(localized: "L'ordinateur est revenu en \(width) × \(height).") : String(localized: "L'ordinateur est passé en \(width) × \(height).")
            if let offer = modeRestoreOffer { text += " " + String(localized: "« \(offer) » dans la barre.") }
            showHint(.notice(text), after: 0, for: 5)
        case let .fallback(level, mbps):
            showHint(.fallback(level == 1 ? String(localized: "L'image ne passait pas : débit ramené à \(mbps) Mbit/s pour cette session.") : String(localized: "L'image ne passait pas : 4:2:0 pour cette session.")), after: 0, for: 6)
        case let .adapting(width, height):
            showHint(.notice(String(localized: "Adaptation de l'image à \(width) × \(height)…")), after: 0, for: 3)
        case let .adapted(width, height):
            requestedBeforeAdapt = nil
            showHint(.notice(String(localized: "Image adaptée à \(width) × \(height).")), after: 0, for: 4)
            // L'écran a pu changer encore pendant l'adaptation.
            checkScreenMode()
        case let .adaptFailed(message):
            // L'ancien mode reste celui demandé (reconnexion, écran virtuel perdu).
            if let previous = requestedBeforeAdapt { requested = previous }
            requestedBeforeAdapt = nil
            showHint(.fallback(String(localized: "L'écran de l'ordinateur n'a pas pu être adapté : \(message)")), after: 0, for: 6)
        case let .restoreOffer(mode):
            modeRestoreOffer = mode.map { String(localized: "Remettre \($0.width) × \($0.height)") }
        case .stationRestarting:
            stationRestarting = true
        }
    }

    // MARK: - Écran de ce Mac

    /// Les écrans changent par rafales (branchement, réveil, plein écran qui
    /// se recale) : on attend qu'ils se posent.
    private func screensChanged() {
        screenCheck?.cancel()
        screenCheck = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(800))
            guard let self, !Task.isCancelled else { return }
            self.screenCheck = nil
            // Le cœur reçoit les écrans à jour : il refait la session si le
            // nombre ou la disposition change (multi-écrans).
            if self.isLive, !self.isGuest, let request = self.refreshDeviceScreens() {
                self.updateExpectedScreens()
                self.engine?.setScreens(request)
            }
            self.refitWindowIfNeeded()
            if !self.multiScreen { self.checkScreenMode() }
        }
    }

    /// Écran de la fenêtre débranché : macOS la passe sur un écran restant,
    /// mais une fenêtre en plein écran peut y garder la taille et la place de
    /// l'ancien. On sort du plein écran, on la pose sur l'écran principal et
    /// on y revient en plein écran. Fenêtre restée hors de tout écran : recentrée.
    private func refitWindowIfNeeded() {
        // Avant l'image, la fenêtre peut être encore en train de passer en plein écran.
        guard let window, !refitting, revealedAt != nil, phase != .ending, phase != .ended else { return }
        let screens = NSScreen.screens
        guard let main = NSScreen.main ?? screens.first else { return }
        if window.styleMask.contains(.fullScreen) {
            if let screen = window.screen, abs(window.frame.width - screen.frame.width) < 2,
               screen.frame.insetBy(dx: -1, dy: -1).contains(window.frame) { return }
            refitting = true
            KyberLog.app.notice("plein écran sur un écran débranché : retour en plein écran sur l'écran restant")
            windowDelegate.onExitFullScreen = { [weak self] in
                guard let self else { return }
                self.windowDelegate.onExitFullScreen = { [weak self] in self?.fullScreenExited() }
                let area = (NSScreen.main ?? main).visibleFrame
                self.window?.setFrame(area.insetBy(dx: area.width * 0.1, dy: area.height * 0.1), display: true)
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    self.refitting = false
                    guard self.phase != .ending, self.phase != .ended else { return }
                    if self.window?.styleMask.contains(.fullScreen) == false { self.window?.toggleFullScreen(nil) }
                }
            }
            window.toggleFullScreen(nil)
            // Sortie du plein écran refusée (transition en cours) : on n'attend pas sans fin.
            DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
                guard let self, self.refitting else { return }
                self.refitting = false
                self.windowDelegate.onExitFullScreen = { [weak self] in self?.fullScreenExited() }
            }
        } else if !screens.contains(where: { $0.frame.intersects(window.frame) }) {
            var frame = window.frame
            frame.size.width = min(frame.width, main.visibleFrame.width)
            frame.size.height = min(frame.height, main.visibleFrame.height)
            frame.origin = CGPoint(x: main.visibleFrame.midX - frame.width / 2, y: main.visibleFrame.midY - frame.height / 2)
            window.setFrame(frame, display: true)
        }
    }

    /// « Comme cet écran » : si le mode de l'écran qui montre la session
    /// (définition réelle, fréquence) n'est plus celui demandé à la console,
    /// on propose d'adapter l'image ; revenu au mode demandé, la proposition part.
    private func checkScreenMode() {
        guard phase == .streaming, requestedBeforeAdapt == nil else { return }
        guard settings.display == .match, virtualDisplay, let r = requested,
              let screen = window?.screen ?? NSScreen.main else {
            clearScreenOffer()
            return
        }
        let m = DisplayInfo.mode(for: screen, codec: host.settings.codec, limit4K: host.settings.limit4K)
        let mode = ScreenMode(width: m.width, height: m.height, refresh: m.refresh)
        guard mode != r else {
            clearScreenOffer()
            return
        }
        guard mode != screenOffer else { return }
        KyberLog.app.notice("écran de ce Mac : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, l'ordinateur est en \(r.width)×\(r.height) à \(r.refresh) Hz : adaptation proposée")
        screenOffer = mode
        screenOfferHide?.cancel()
        withAnimation(Theme.easeOut) { screenOfferShown = true }
        // La capsule s'efface ; la puce reste dans la barre.
        screenOfferHide = Task { [weak self] in
            try? await Task.sleep(for: .seconds(15))
            guard !Task.isCancelled else { return }
            self?.dismissScreenOffer()
        }
    }

    private func clearScreenOffer() {
        screenOfferHide?.cancel()
        screenOffer = nil
        if screenOfferShown { withAnimation(.easeIn(duration: 0.25)) { screenOfferShown = false } }
        screenOfferRect = .zero
    }

    /// « Plus tard » : la capsule part, la puce reste dans la barre.
    func dismissScreenOffer() {
        screenOfferHide?.cancel()
        withAnimation(.easeIn(duration: 0.25)) { screenOfferShown = false }
        screenOfferRect = .zero
    }

    /// « Adapter » : le cœur fait refaire l'écran virtuel au mode de cet écran
    /// par la console, puis relance l'image dessus (avis « adaptation »,
    /// « adaptée » ou « refusée »).
    func adaptToScreen() {
        guard let offer = screenOffer, let engine, phase == .streaming, requestedBeforeAdapt == nil else { return }
        clearScreenOffer()
        requestedBeforeAdapt = requested
        requested = offer
        KyberLog.app.notice("adaptation à l'écran de ce Mac : \(offer.width)×\(offer.height) à \(offer.refresh) Hz demandé")
        engine.setMode(SessionEngine.Mode(width: offer.width, height: offer.height, refresh: offer.refresh))
    }

    // MARK: - Mode et écrans de l'ordinateur

    /// « Remettre » : la console remet le mode demandé au début de la session.
    func restoreMode() {
        modeRestoreOffer = nil
        engine?.restoreMode()
    }

    /// Sans écran virtuel : afficher un autre écran de l'ordinateur.
    var canPickScreen: Bool { !virtualDisplay && screens.count > 1 }

    func showScreen(_ id: UInt32) {
        guard id != shownDisplay else { return }
        engine?.pickDisplay(id)
    }

    // MARK: - Fin

    /// Fin de session : l'image se referme, la console rallume les écrans de
    /// l'ordinateur et rétablit son son, l'app affiche ce qui a été vérifié.
    func end() {
        guard phase != .ending && phase != .ended else { return }
        setBar(false)
        // La fin ferme toutes les fenêtres : la connexion se referme sur l'écran principal.
        if let main = mainSurface { moveChrome(to: main) }
        extraSurfaces.forEach(closeSurface)
        // La dernière image : gardée pour l'accueil, et figée sous le passage qui se referme.
        let last = engine?.snapshot(maxWidth: 1600)
        // Un invité ne laisse rien sur cet appareil : ni dernière image, ni statistiques.
        if let last, !isGuest { store.imagery.saveLast(last, for: host) }
        recordSession()
        // Les moitiés se referment, puis la fenêtre se ferme tout de suite ; la
        // console vérifie la remise en état en arrière-plan (onReport).
        scene.close(over: revealedAt != nil ? last : nil, home: homeImage) { [weak self] in self?.finish() }
        withAnimation(.easeInOut(duration: 0.3)) { phase = .ending }
        clearScreenOffer()
        streamView.detach()
        engine?.stop()
        stats = nil
        audioStats = nil
        statsPanel = false
        statsCloseRect = .zero
        #if DEBUG
        // Démonstration, sans cœur derrière : le bilan d'Admin un instant après.
        if engine == nil, AppModel.demo {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                guard let self else { return }
                self.closing.done = true
                self.coreEnded = true
                self.onEnded?(self)
            }
        }
        #endif
    }

    /// Dernière session (rubrique Statistiques) : médiane et 95e centile de
    /// la latence (le cœur les donne aussi à la console), durée, mode, format
    /// et débit demandés ; gardée si elle a duré au moins 3 s.
    private func recordSession() {
        guard let revealedAt, !isGuest else { return }
        let m = streamMode
        let latency = engine?.sessionLatency, codec = host.settings.codec, mbps = sessionMbps, screens = screenCount
        commit { $0.recordSession(start: revealedAt, latency: latency, width: m.width, height: m.height, refresh: m.refresh,
                                  codec: codec, mbps: mbps, screens: screens) }
    }

    /// Débit de la session en Mbit/s : avec plusieurs écrans, sur la somme
    /// de leurs pixels par seconde (le débit total, comme le cœur).
    private var sessionMbps: Int {
        let m = streamMode
        guard screenCount > 1, let request = screensRequest else {
            return host.settings.bitrate(width: m.width, height: m.height, refresh: m.refresh)
        }
        let plan = SessionEngine.sessionPlan(request, profile: host.settings.profile?.rawValue,
                                             virtualDisplay: virtualDisplay, codec: host.settings.codec) ?? []
        let rate = plan.reduce(0.0) { $0 + Double($1.width * $1.height * $1.refresh) }
        guard rate > 0, host.settings.profile != nil else { return host.settings.bitrate(width: m.width, height: m.height, refresh: m.refresh) }
        let scale = pow(rate / ConnectionSettings.referencePixelRate, 0.75)
        let mbps = (Double(host.settings.bitrateMbps) * min(max(scale, 0.4), 3)).rounded()
        return min(max(Int(mbps), ConnectionSettings.minBitrate), ConnectionSettings.maxBitrate)
    }

    /// Résultat de la remise en état par la console, après le retour à l'accueil.
    var onReport: ((Host, Closing) -> Void)?

    private func showHint(_ which: Hint, after delay: TimeInterval = 1.6, for duration: TimeInterval = 3.4) {
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            withAnimation(.easeOut(duration: 0.35)) { self?.hint = which }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + delay + duration) { [weak self] in
            guard self?.hint == which else { return }
            withAnimation(.easeIn(duration: 0.35)) { self?.hint = nil }
        }
    }

    /// ⌃⌥⌘S : panneau des mesures détaillées.
    func toggleStatsPanel() {
        withAnimation(Theme.easeOut) { statsPanel.toggle() }
        if !statsPanel { statsCloseRect = .zero }
    }

    /// ⌃⌥⌘M : mode jeu si la souris est en mode bureau, et inversement.
    func toggleMouseCapture() {
        setMouse(streamView.isRelative ? .desktop : .game)
    }

    /// Le clavier et la souris de ce Mac partent vers l'ordinateur : une
    /// session d'hôte, ou un invité à qui l'hôte les a donnés.
    var hasKeyboardAndMouse: Bool { !isGuest || shareRole.keyboardAndMouse }

    /// Choix de la barre ou de ⌃⌥⌘M, gardé pour cet ordinateur.
    func setMouse(_ choice: MouseChoice) {
        // Un invité sans le clavier et la souris n'a pas de souris à régler.
        guard hasKeyboardAndMouse else { return }
        commit { $0.settings.mouse = choice }
        streamView.mouseMode = choice.streamMode
    }

    /// Ce que la session change de son ordinateur (profil, souris, dernière
    /// session) : sur sa copie, et sur celle du magasin telle qu'elle est
    /// maintenant (l'accueil et les réglages restent ouverts pendant les
    /// sessions : ce qu'on y a changé n'est pas écrasé).
    private func commit(_ change: (inout Host) -> Void) {
        change(&host)
        guard var latest = store.host(host.id) else { return }
        change(&latest)
        store.update(latest)
    }

    /// Fin de la session côté interface : le gestionnaire la retire, puis sa
    /// fenêtre se ferme (en plein écran, une fois sortie, sur l'accueil
    /// photographié, comme elle s'était ouverte).
    func finish() {
        guard phase != .ended else { return }
        phase = .ended
        teardown()
        onFinish(self)
        closeWindow()
    }

    private func teardown() {
        windowDelegate.onClose = nil
        compositing = nil
        sceneHide?.cancel()
        // Seules les moitiés du passage restent à l'écran pendant la fermeture.
        chrome?.removeFromSuperview()
        chrome = nil
        if let occlusionObserver { NotificationCenter.default.removeObserver(occlusionObserver) }
        occlusionObserver = nil
        screenObservers.forEach { NotificationCenter.default.removeObserver($0) }
        screenObservers = []
        activeObservers.forEach { NotificationCenter.default.removeObserver($0) }
        activeObservers = []
        screenCheck?.cancel()
        screenOfferHide?.cancel()
        window?.makeFirstResponder(nil)
        extraSurfaces.forEach(closeSurface)
        mainSurface?.close()
    }

    /// La fenêtre de la session se ferme ; en plein écran, après en être
    /// sortie (3 s au plus : une sortie refusée pendant une transition).
    private func closeWindow() {
        guard let window else { return }
        guard window.styleMask.contains(.fullScreen) else { return dropWindow() }
        windowDelegate.onExitFullScreen = { [weak self] in self?.dropWindow() }
        window.toggleFullScreen(nil)
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in self?.dropWindow() }
    }

    private func dropWindow() {
        guard let window else { return }
        self.window = nil
        windowDelegate.onExitFullScreen = nil
        window.delegate = nil
        window.orderOut(nil)
        window.close()
        onWindowClosed?(self)
    }

    /// Appelé une fois la fenêtre fermée : l'app ramène l'accueil après la
    /// dernière session (AppLife.swift).
    var onWindowClosed: ((SessionController) -> Void)?

    /// Session finie en erreur : l'image s'assombrit, le message reste jusqu'à
    /// « Fermer » ; le cœur a prévenu la console.
    private func fail(_ message: String) {
        KyberLog.app.error("session : \(message, privacy: .public)")
        // Une session coupée après l'image compte aussi.
        if revealedAt != nil { recordSession() }
        clearScreenOffer()
        engine?.stop()
        stats = nil
        audioStats = nil
        statsPanel = false
        statsCloseRect = .zero
        setBar(false)
        // L'échec se lit sur l'écran principal ; les autres fenêtres se ferment.
        if let main = mainSurface { moveChrome(to: main) }
        extraSurfaces.forEach(closeSurface)
        replanning = false
        streamView.detach()
        scene.dim()
        phase = .failed(message)
    }

    func dismissFailure() {
        finish()
    }

    #if DEBUG
    /// Démonstration (`-demo YES -screen connecting|failed|session`) : la
    /// fenêtre de session sans ordinateur derrière, à l'étape demandée.
    func demo(_ screen: String) {
        openWindow()
        // KyberNow Share : le partage se joue ici, sans Station.
        share.demo = true
        modeLabel = "2560 × 1440 · 120 Hz"
        switch screen {
        case "failed":
            phase = .connecting
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in
                self?.scene.dim()
                self?.phase = .failed(String(localized: "la Station ne répond pas"))
            }
        case "restarting":
            phase = .connecting
            stationRestarting = true
        case "session", "bar", "stats":
            phase = .starting
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in
                guard let self else { return }
                self.revealedAt = Date()
                self.phase = .streaming
                self.scene.open()
                if screen == "bar" { self.setBar(true) }
                if screen == "stats" { self.toggleStatsPanel() }
                self.demoStats()
            }
        default:
            phase = .connecting
        }
    }

    /// Démonstration : des mesures qui bougent, une fois par seconde (barre,
    /// menu de la barre des menus ; mode commun, comme les mesures du cœur
    /// qui arrivent par la file principale, aussi menu ouvert).
    private func demoStats() {
        let timer = Timer(timeInterval: 1, repeats: true) { [weak self] timer in
            MainActor.assumeIsolated {
                guard let self, self.phase == .streaming else { timer.invalidate(); return }
                var s = VideoStats()
                s.latencyMs = 11.5 + Double.random(in: 0 ... 1.6)
                s.fps = 60 + Double.random(in: -0.4 ... 0.4)
                s.mbps = 80 + Double.random(in: -3 ... 3)
                s.width = 2560
                s.height = 1440
                s.format = "4:2:0 8 bits"
                self.stats = s
                self.latencyHistory.append(s.latencyMs ?? 0)
                if self.latencyHistory.count > 40 { self.latencyHistory.removeFirst() }
            }
        }
        RunLoop.main.add(timer, forMode: .common)
    }

    /// Démonstration sans fenêtre : la session « en image », sa barre ouverte,
    /// pour la dessiner dans un PNG (`bar-render`, SharePanel.swift) sans rien
    /// ouvrir à l'écran.
    func demoHeadless() {
        share.demo = true
        phase = .streaming
        revealedAt = Date()
        modeLabel = "2560 × 1440 · 120 Hz"
        var s = VideoStats()
        s.latencyMs = 11.8
        s.fps = 120
        s.mbps = 80
        s.width = 2560
        s.height = 1440
        s.format = "4:2:0 8 bits"
        stats = s
        latencyHistory = (0..<40).map { 11 + Double($0 % 7) * 0.35 }
        setBar(true)
    }

    /// Démonstration : la session d'un invité (sans Station), à l'étape demandée
    /// (`-demoGuest spectator|player|player_kbm`).
    func demoGuest(role: ShareRole) {
        demo("session")
        shareRole = role
    }

    /// Démonstration : l'image que reçoit l'invité (nil : encore inconnue), avec
    /// l'avis du cœur s'il y en a un (`moment-render:lag|limit`).
    func demoShareImage(_ image: ShareImage?, notice: String? = nil) {
        guard isGuest else { return }
        if let image { shareImageChanged(image, notice: notice) } else { shareImage = nil }
    }

    /// Démonstration : l'hôte change le rôle de l'invité.
    func demoRoleChanged(_ role: ShareRole) {
        guard isGuest else { return }
        let previous = shareRole
        shareRoleChanged(role, previous: previous)
    }

    /// Démonstration : l'hôte arrête le partage ou retire l'invité.
    func demoShareEnded(removed: Bool) {
        guard isGuest, isLive else { return }
        let host = guest?.host ?? "Léa"
        shareEnded(removed ? String(localized: "\(host) vous a retiré du partage.") : String(localized: "\(host) a arrêté le partage."))
        endedByComputer = true
        fail(shareEndText ?? "")
    }

    /// Démonstration : fin venue de l'ordinateur (Admin, un autre appareil),
    /// ou erreur (`error`), comme le cœur les dit.
    func demoEndedByComputer(error: String?) {
        guard isLive else { return }
        endedByComputer = true
        endError = error
        end()
    }
    #endif
}
