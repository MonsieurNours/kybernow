// Une session avec un ordinateur, sur l'iPhone ou l'iPad, vue de
// l'interface : le passage, l'écran de connexion et ses trois étapes, la
// barre (languette, glissement depuis le bord haut, ou L1 + R1 + Start +
// Select à la manette), le clavier et sa rangée de touches, le menu Touches,
// les statistiques, les capsules des événements. Dès la première image,
// l'image seule : rien ne s'affiche de lui-même à l'arrivée. Le déroulé
// lui-même (préparation par Admin, connexion à Kyber, choix de l'écran,
// relances, reconnexions, repli, signal de vie, fin vérifiée) est dans le
// cœur Rust, par `SessionEngine`, comme sur Mac et Apple TV
// (docs/coeur-apple.md).
//
// Manette à l'écran : bouton de la barre, retenue pour chaque ordinateur
// (`host.settings.screenPad`) ; affichée, elle prend une place de joueur
// (GamepadBridge) et le toucher ne sert plus de souris. Le clavier de
// l'écran se replie d'un toucher sur l'image (StreamView).
//
// Écran externe : l'image y passe, à sa définition, et l'appareil devient le
// pupitre (pavé tactile sur toute la surface, barre permanente, mesures,
// Touches et clavier sans jamais couvrir l'image) ; débranché, l'image
// revient sur l'appareil sans couper la session. Stage Manager : la session
// vit dans sa fenêtre ; perdre le premier plan relâche tout sur
// l'ordinateur et ramène le débit, comme l'arrière-plan.

import AVFoundation
import Combine
import GameController
import KyberKit
import SwiftUI
import UIKit

@MainActor
final class SessionController: ObservableObject {
    enum Phase: Equatable {
        case preparing, connecting, starting, streaming
        case reconnecting(Int)
        case ending
        case ended
        case failed(String)
    }

    @Published private(set) var phase: Phase = .preparing
    @Published private(set) var host: Host
    /// Instant où l'image est apparue (le passage s'ouvre).
    @Published private(set) var revealedAt: Date?
    /// Dernier recours du cœur : l'image ne venait pas, il redémarre la
    /// Station et attend son retour (« La Station redémarre… » sur l'écran de
    /// connexion), jusqu'à l'état suivant.
    @Published private(set) var stationRestarting = false
    /// Barre de session ouverte.
    @Published private(set) var barOpen = false
    /// La manette pilote la barre (L1 + R1 + Start + Select).
    @Published private(set) var padMode = false
    @Published private(set) var stats: VideoStats?
    @Published private(set) var audioStats: AudioStats?
    /// Latences des dernières secondes (courbe de la barre).
    @Published private(set) var latencyHistory: [Double] = []
    /// Mesures des 30 dernières secondes (feuille des mesures).
    @Published private(set) var statsHistory: [VideoStats] = []
    /// Pertes depuis le début de la session.
    @Published private(set) var sessionLosses = (frames: 0, packets: 0)
    @Published private(set) var route = AudioRoute.current
    @Published private(set) var modeLabel = ""
    @Published private(set) var startedAt = Date()
    /// Capsule d'information en haut de l'image (mode changé, image relancée…).
    @Published private(set) var notice: String?
    /// La capsule se choisit : elle ouvre le panneau du partage (la demande d'un invité, « Léa demande à rejoindre. »).
    @Published private(set) var noticeOpensShare = false
    /// La capsule est une demande d'emprunt : la choisir ouvre le panneau du prêt.
    @Published private(set) var noticeOpensLend = false
    @Published private(set) var fallbackNote: String?
    /// Profil Bureau, écran de l'ordinateur passé à un autre mode : « Remettre 2622 × 1206 ».
    @Published private(set) var modeRestoreOffer: String?
    /// Clavier de l'écran ouvert.
    @Published private(set) var keyboardShown = false
    /// Manette à l'écran affichée (retenu pour cet ordinateur).
    @Published private(set) var screenPad = false
    /// Ctrl, Alt, ⊞ collants allumés (rangée de touches).
    @Published private(set) var sticky: Set<UInt16> = []
    /// Feuilles : menu Touches, statistiques.
    @Published var keysMenu = false
    @Published var statsSheet = false
    /// Écrans de l'ordinateur (choix dans la barre, sans écran virtuel).
    @Published private(set) var screens: [KyberDisplay] = []
    @Published private(set) var shownDisplay: UInt32?
    @Published private(set) var virtualDisplay = true
    /// L'image est sur l'écran externe : l'appareil est le pupitre.
    @Published private(set) var external = false

    let streamView = StreamView(frame: CGRect(x: 0, y: 0, width: 874, height: 402))
    private let store: HostStore
    private let onFinish: (SessionController) -> Void
    private let prefs = Preferences.shared
    private var engine: SessionEngine?
    private var engineStreaming = false
    /// Mode de l'écran de la session (demandé, puis celui obtenu de l'ordinateur) :
    /// gardé avec la dernière session.
    private var streamMode = (width: 0, height: 0, refresh: 0)
    private var observers: [NSObjectProtocol] = []
    private var externalWatch: AnyCancellable?
    private static let revealDelay: TimeInterval = 0.9
    /// Arrière-plan : débit plafonné, image figée (la session continue).
    private static let backgroundBitrate: UInt32 = 8_000_000

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
    /// La raison de la fin par le partage (« {hôte} a arrêté le partage. »).
    @Published private(set) var shareEndText: String?
    /// Le partage de cette session d'hôte (« Partager » dans la barre).
    let share: ShareHostModel
    /// Le panneau de l'hôte (code, QR code, invités) est ouvert.
    @Published var shareSheet = false

    /// Par où passe la session (docs/relais.md) : direct sur le réseau, ou par le
    /// relais hors de chez soi (puce « Par le relais » dans la barre, ligne
    /// « Chemin » des statistiques). Donné par le cœur, au départ puis quand il change.
    @Published private(set) var path = RouteInfo.direct

    // MARK: Prêt de l'ordinateur

    /// Session d'emprunt (docs/pret.md) : d'où vient l'emprunt (la Station, le
    /// code ou le compte, le prénom), gardé pour « Revenir ». Comme un invité,
    /// un ami ne change rien sur l'ordinateur et ne garde rien sur cet appareil ;
    /// il a le clavier, la souris et les manettes, sans presse-papiers.
    let borrowSource: BorrowSource?
    var isBorrower: Bool { borrowSource != nil }
    /// Un invité ou un ami qui emprunte : ni profil, ni réglages, ni dernière image.
    var isVisitor: Bool { isGuest || isBorrower }
    /// L'emprunt avant la session : « Préparation de l'ordinateur », « En attente de {hôte}… ».
    let borrowFlow: BorrowFlow?
    /// Le prêt obtenu : la fin prévue (« Encore 1 h 20 ») et {hôte}.
    @Published private(set) var loan: LendLoan?
    /// Le préavis de 2 minutes en cours (le temps restant le suit).
    @Published private(set) var lendNotice: LendNotice?
    /// La fin par le prêt (« {hôte} a repris l'ordinateur. ») ou un refus de
    /// l'emprunt : elle reste à l'écran, avec « Fermer ».
    @Published private(set) var lendEndText: String?
    /// « Rendre l'ordinateur à {hôte} ? » est posé.
    @Published var giveBackAsked = false
    /// L'ami a demandé « Rendre l'ordinateur » : la fin est voulue.
    private var givingBack = false
    /// {hôte} des textes du prêt.
    var lendHost: String { loan?.host ?? borrowFlow?.host.nonEmpty ?? borrowSource?.hostHint.nonEmpty ?? host.name }
    /// La session d'emprunt s'est coupée sans fin du prêt (réseau perdu) : l'ami
    /// peut revenir pendant 15 minutes (« Revenir »).
    var canComeBack: Bool { isBorrower && loan != nil && lendEndText == nil && !givingBack }
    /// « Revenir » : l'app ouvre le même emprunt dans une nouvelle session.
    var onComeBack: ((BorrowSource) -> Void)?
    /// Le prêt de cet ordinateur, depuis la session de l'hôte (« Prêter l'ordinateur… »).
    let lend: LendHostModel
    /// Le panneau « Prêter l'ordinateur… » est ouvert.
    @Published var lendSheet = false

    /// Le titre de la session : le nom de l'ordinateur, « Invité sur {nom} »,
    /// ou « Ordinateur de {hôte} ».
    var title: String {
        if isBorrower { return LendText.computerOf(lendHost) }
        return guest.map { ShareText.guestOn($0.computer) } ?? host.name
    }

    /// « Partager » avec cet ordinateur : disponible, ou inactif avec la raison.
    var shareAvailability: ShareAvailability { share.availability }

    init(host: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.host = host
        self.store = store
        self.onFinish = onFinish
        guest = nil
        borrowSource = nil
        borrowFlow = nil
        share = ShareHostModel(console: nil) { [store, id = host.id] in ShareHostModel.availability(of: id, in: store) }
        lend = LendHostModel(console: nil) { [store, id = host.id] in store.lendAvailability(of: id) }
    }

    /// Une session d'invité : une Station qui a accepté un code (`Share.join`).
    init(guest found: ShareFound, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
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
        self.onFinish = onFinish
        guest = found
        shareRole = found.role
        shareQuality = found.quality
        virtualDisplay = false
        borrowSource = nil
        borrowFlow = nil
        share = ShareHostModel(console: nil) { .available }
        lend = LendHostModel(console: nil) { .available }
    }

    /// Une session d'emprunt : l'ordinateur d'un ami, par un code de prêt ou un
    /// compte d'ami. L'emprunt se fait au départ de la session (`start`).
    init(borrow source: BorrowSource, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        let name = LendText.computerOf(source.hostHint.isEmpty ? source.target.host : source.hostHint)
        var host = Host(name: name, address: source.target.host, consolePort: source.target.port, username: "")
        // L'écran virtuel que la Station fait à la taille de cet appareil, profil
        // Jeu, sans presse-papiers ; rien n'est gardé ensuite.
        host.settings.apply(.jeu)
        host.settings.display = .match
        host.settings.clipboard = false
        host.settings.keepLastImage = false
        self.host = host
        self.store = store
        self.onFinish = onFinish
        guest = nil
        borrowSource = source
        borrowFlow = BorrowFlow(source: source)
        share = ShareHostModel(console: nil) { .available }
        lend = LendHostModel(console: nil) { .available }
    }

    /// Les réglages de l'image, pour le cœur. AV1 seulement si l'appareil le décode.
    private var engineSettings: SessionEngine.Settings {
        let s = host.settings
        var codec = s.codec
        if codec == .av1, !DecoderCapabilities.current.av1 { codec = DecoderCapabilities.current.hevc ? .hevc : .h264 }
        return SessionEngine.Settings(
            profile: s.profile?.rawValue,
            codec: codec,
            yuv444: s.yuv444 && ConnectionSettings.decodes444(codec),
            mbps: s.bitrateMbps,
            reliable: s.transport == .reliable
        )
    }

    // MARK: - Déroulé

    func start() {
        // Un invité n'a pas de compte : le code a suffi (un ami : le prêt).
        let password = isVisitor ? "" : store.password(for: host)
        guard let password else {
            fail(String(localized: "mot de passe introuvable dans le Trousseau"))
            return
        }
        let settings = host.settings
        // « Comme cet écran » : l'écran externe s'il est branché, sinon celui de l'appareil.
        let mode = ExternalDisplay.shared.mode(for: settings) ?? DeviceDisplay.mode(for: settings, screen: deviceScreen)
        modeLabel = "\(mode.width) × \(mode.height) · \(mode.refresh) Hz"
        streamMode = mode
        configureView()
        observeApp()
        if ExternalDisplay.shared.scene != nil { useExternal(true, adapt: false) }
        externalWatch = ExternalDisplay.shared.$scene.dropFirst().sink { [weak self] scene in
            self?.useExternal(scene != nil, adapt: true)
        }

        // Prêt de l'ordinateur : l'emprunt d'abord (« Préparation de
        // l'ordinateur », ou « En attente de {hôte}… »), avec l'écran de cet
        // appareil ; la session se connecte une fois le prêt obtenu.
        if let flow = borrowFlow, loan == nil {
            phase = .preparing
            flow.onReady = { [weak self] loan in
                guard let self, self.phase == .preparing else {
                    Lend.leave(loan)
                    return
                }
                self.loan = loan
                self.host.name = LendText.computerOf(loan.host)
                self.startEngine(mode: mode, password: password)
            }
            flow.onFailed = { [weak self] text in
                guard let self else { return }
                self.lendEndText = text
                self.fail(text)
            }
            flow.start(screen: LendScreen(width: mode.width, height: mode.height, refresh: mode.refresh))
            return
        }
        startEngine(mode: mode, password: password)
    }

    /// La session du cœur : celle de l'ordinateur, d'un invité ou d'un ami qui emprunte.
    private func startEngine(mode: (width: Int, height: Int, refresh: Int), password: String) {
        let settings = host.settings
        let engine = SessionEngine(view: streamView)
        engine.onPhase = { [weak self] in self?.phaseChanged($0) }
        engine.onNotice = { [weak self] in self?.notice($0) }
        engine.onDisplays = { [weak self] in self?.screens = $0 }
        engine.onShownDisplay = { [weak self] in self?.shownDisplay = $0.id }
        engine.onMode = { [weak self] mode, virtualDisplay in
            guard let self else { return }
            self.modeLabel = "\(mode.width) × \(mode.height) · \(mode.refresh) Hz"
            self.streamMode = (mode.width, mode.height, mode.refresh)
            self.virtualDisplay = virtualDisplay
        }
        engine.onVideoStats = { [weak self] in self?.received($0) }
        engine.onAudioStats = { [weak self] in self?.audioStats = $0 }
        engine.onGamepadCombo = { [weak self] in self?.gamepadCombo() }
        engine.onGameFullscreen = { game in
            KyberLog.app.notice("ordinateur : \(game ? "jeu en plein écran" : "pas de jeu en plein écran", privacy: .public)")
        }
        // KyberNow Share : la liste des invités (hôte), le rôle et la fin (invité).
        engine.onGuests = { [weak self] in self?.share.heartbeat($0) }
        share.onRequest = { [weak self] in self?.requestArrived($0) }
        engine.onShareRole = { [weak self] role, previous in self?.shareRoleChanged(role, previous: previous) }
        engine.onShareEnd = { [weak self] _, text in self?.shareEndText = text }
        engine.onShareImage = { [weak self] image, _, notice in self?.shareImageChanged(image, notice: notice) }
        // Prêt de l'ordinateur : la vue du prêt (hôte), le préavis et la fin (ami).
        engine.onLend = { [weak self] view in
            guard let self else { return }
            self.lend.heartbeat(view)
            self.store.setLend(view, for: self.host.id)
        }
        lend.onRequest = { [weak self] in self?.lendRequestArrived($0) }
        engine.onLendNotice = { [weak self] notice, text in self?.lendNoticed(notice, text: text) }
        engine.onLendEnd = { [weak self] end, text in self?.lendEnded(end, text: text) }
        engine.onRoute = { [weak self] in self?.routeChanged($0) }
        self.engine = engine
        if !isVisitor {
            share.setConsole(store.console(for: host))
            lend.setConsole(store.console(for: host))
        }

        // Son : celui de la sortie (AirPods en audio spatial, ampli HDMI), ou resté sur l'ordinateur.
        let channels = settings.audio == .off ? 0 : (settings.audio == .auto ? AudioOutput.channels() : settings.audio.channels)
        var params: SessionEngine.Params
        if let loan {
            // Un ami : la Station du prêt, sur l'écran virtuel qu'elle a fait pour cet appareil.
            params = SessionEngine.Params(borrower: loan, audioChannels: channels, settings: engineSettings)
        } else if let guest {
            // Un invité : la Station qui a accepté le code, sans compte ni écran virtuel.
            params = SessionEngine.Params(guest: guest, audioChannels: channels, settings: engineSettings)
        } else {
            params = SessionEngine.Params(
                host: host.address, consolePort: host.consolePort, username: host.username, password: password,
                consoleFingerprint: host.consoleFingerprint,
                mode: SessionEngine.Mode(width: mode.width, height: mode.height, refresh: mode.refresh),
                audioChannels: channels, virtualDisplay: settings.display != .physical, settings: engineSettings
            )
            params.clipboard = settings.clipboard
            // Hors de chez soi : le réseau d'abord, puis le relais si l'accès est actif.
            params.relay = host.sessionRelay
        }
        // Par le relais, une fin faute de joindre l'ordinateur le dit avec son nom (l'ordinateur du partage pour un invité).
        params.computerName = host.name
        params.autoReconnect = settings.autoReconnect
        params.gamepadAsXbox360 = settings.gamepad == .xbox360
        params.vsync = settings.vsync
        KyberLog.app.notice("session : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, son \(channels) canaux, \(settings.consoleProfile, privacy: .public)")
        // Joignable par le relais seulement (ou relais forcé pour essayer) : la radio
        // cellulaire s'allume d'abord, par une requête de l'app vers le relais.
        if let relay = params.relay, store.status[host.id]?.away == true || Relay.mode == .force {
            Task { @MainActor [weak self] in
                await Relay.wakeRadio(server: relay.server)
                guard let self, self.phase == .preparing else { return }
                self.launch(engine, params)
            }
        } else {
            launch(engine, params)
        }
    }

    private func launch(_ engine: SessionEngine, _ params: SessionEngine.Params) {
        do {
            try engine.start(params)
        } catch {
            fail(error.localizedDescription)
        }
    }

    // MARK: - Hors de chez soi

    /// Le chemin de la session a changé (au départ, puis toutes les 5 s au plus
    /// s'il change) : la puce « Par le relais » suit. Par le relais, le partage de
    /// l'hôte passe aussi par lui (Admin n'est plus joignable sur le réseau).
    private func routeChanged(_ info: RouteInfo) {
        switch phase {
        case .ending, .ended, .failed: return
        default: break
        }
        guard info != path else { return }
        let wasRelay = path.viaRelay
        path = info
        if info.viaRelay != wasRelay, !isVisitor {
            share.setConsole(store.console(for: host, route: info.viaRelay ? .relay : .direct))
            lend.setConsole(store.console(for: host, route: info.viaRelay ? .relay : .direct))
            // Par le relais, l'app ne redémarre ni ne met à jour la Station (réglages).
            store.setSessionViaRelay(host, info.viaRelay)
        }
    }

    /// L'écran de l'appareil (celui de la fenêtre de l'app).
    private var deviceScreen: UIScreen {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.session.role == .windowApplication }?.screen ?? UIScreen.main
    }

    private var deviceName: String {
        DeviceDisplay.isPad ? String(localized: "l'iPad") : String(localized: "l'iPhone")
    }

    /// L'image passe sur l'écran externe, ou revient sur l'appareil. En cours
    /// de session (`adapt`), Admin refait l'écran virtuel à la taille du
    /// nouvel écran, puis l'image repart dessus (avis « adaptation »).
    private func useExternal(_ on: Bool, adapt: Bool) {
        guard on != external, phase != .ending, phase != .ended else { return }
        let display = ExternalDisplay.shared
        external = on
        streamView.showImage(in: on ? display.imageHost : nil)
        display.showingImage(on)
        if on {
            setBar(true)
            if DeviceDisplay.isPad { statsSheet = true }
        } else {
            statsSheet = false
        }
        guard adapt else { return }
        show(notice: on ? String(localized: "L'image passe sur l'écran externe") : String(localized: "L'image est revenue sur \(deviceName)"))
        let mode = on ? display.mode(for: host.settings) : DeviceDisplay.mode(for: host.settings, screen: deviceScreen)
        if let mode, virtualDisplay, host.settings.display != .custom {
            engine?.setMode(SessionEngine.Mode(width: mode.width, height: mode.height, refresh: mode.refresh))
        }
    }

    private func configureView() {
        streamView.directTouch = prefs.directTouch
        streamView.pointerSpeed = prefs.pointerSpeed
        streamView.appleISOKeyboard = prefs.appleISOKeyboard
        streamView.commandAsControl = host.settings.commandAsControl
        streamView.mouseMode = host.settings.mouse.streamMode
        streamView.onTap = { [weak self] in self?.setBar(false) }
        streamView.onTopEdgeSwipe = { [weak self] in self?.setBar(true) }
        streamView.onStickyChanged = { [weak self] in self?.sticky = $0 }
        let gamepads = GamepadBridge.shared
        gamepads.onMenuRequest = { [weak self] in self?.gamepadCombo() }
        gamepads.menuFromCombo = prefs.menuCombo
        gamepads.rumbleEnabled = prefs.rumble
        // La manette à l'écran de la dernière fois : elle prend sa place tout
        // de suite, et s'annonce dès que les entrées rejoignent l'ordinateur.
        if host.settings.screenPad { showScreenPad(true) }
    }

    /// La fenêtre de la session perd le premier plan (autre app, verrouillage,
    /// autre fenêtre dans Stage Manager) : rien ne reste enfoncé sur
    /// l'ordinateur, le débit est plafonné, l'image se fige ; au retour, tout
    /// revient. Suivi par scène : avec Stage Manager, l'app peut rester
    /// visible sans avoir le clavier ni le toucher.
    private func observeApp() {
        let center = NotificationCenter.default
        observers = [
            center.addObserver(forName: UIScene.willDeactivateNotification, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, self.isSessionScene(note.object), self.phase == .streaming else { return }
                    KyberLog.app.notice("fenêtre de la session inactive : image figée")
                    self.streamView.releaseAll()
                    self.engine?.setBitrateCap(Self.backgroundBitrate)
                    // Sur l'écran externe, l'image reste vivante : on la regarde encore.
                    if !self.external { self.engine?.setOccluded(true) }
                    self.show(notice: String(localized: "KyberNow Link continue en arrière-plan"), for: 4)
                }
            },
            center.addObserver(forName: UIScene.didActivateNotification, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, self.isSessionScene(note.object) else { return }
                    KyberLog.app.notice("fenêtre de la session active : image reprise")
                    self.engine?.setOccluded(false)
                    self.engine?.setBitrateCap(nil)
                }
            },
            center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.route = AudioRoute.current }
            },
            center.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, let c = note.object as? GCController, c.extendedGamepad != nil else { return }
                    self.show(notice: String(localized: "\(c.vendorName ?? String(localized: "Manette")) branchée"), for: 3)
                }
            },
        ]
    }

    /// La scène de la fenêtre de la session (pas celle de l'écran externe).
    private func isSessionScene(_ object: Any?) -> Bool {
        guard let scene = object as? UIWindowScene, scene.session.role == .windowApplication else { return false }
        return streamView.window?.windowScene.map { $0 === scene } ?? true
    }

    private func received(_ stats: VideoStats) {
        self.stats = stats
        if let ms = stats.latencyMs {
            latencyHistory.append(ms)
            if latencyHistory.count > 40 { latencyHistory.removeFirst() }
        }
        statsHistory.append(stats)
        if statsHistory.count > 30 { statsHistory.removeFirst() }
        sessionLosses.frames += stats.lostFrames
        sessionLosses.packets += stats.lostPackets
    }

    /// Les états du cœur : l'image n'apparaît qu'après `revealDelay`.
    private func phaseChanged(_ p: SessionEngine.Phase) {
        if case .failed = phase { return }
        engineStreaming = p == .streaming
        if p != .connecting, stationRestarting { stationRestarting = false }
        switch p {
        case .preparing:
            phase = .preparing
        case .connecting:
            phase = .connecting
        case .starting:
            if revealedAt == nil { phase = .starting }
        case .streaming:
            guard revealedAt == nil else {
                phase = .streaming
                return
            }
            phase = .starting
            DispatchQueue.main.asyncAfter(deadline: .now() + Self.revealDelay) { [weak self] in
                guard let self, self.revealedAt == nil, self.engineStreaming, self.engine != nil else { return }
                self.reveal()
            }
        case let .reconnecting(attempt, _):
            phase = .reconnecting(attempt)
        case .ending:
            // Un invité dont l'hôte a arrêté ou retiré le partage : la raison
            // reste à l'écran (jamais « Reprendre la main »).
            if isGuest, revealedAt != nil, let reason = shareEndText, phase != .ending, phase != .ended {
                fail(reason)
                return
            }
            // Un ami : la fin du prêt (« {hôte} a repris l'ordinateur. ») reste à
            // l'écran, avec « Fermer » ; rendu, la session se ferme.
            if isBorrower, let reason = lendEndText, phase != .ending, phase != .ended {
                fail(reason)
                return
            }
            if phase != .ending, phase != .ended { end() }
        case let .ended(error, _):
            if let error, phase != .ending, phase != .ended {
                fail(error)
                return
            }
            if phase != .ending, phase != .ended { end() }
        }
    }

    /// La première image : le passage s'ouvre sur elle, et rien d'autre.
    private func reveal() {
        withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.2)) {
            revealedAt = Date()
            phase = .streaming
        }
        startedAt = Date()
        host.lastSession = Date()
        store.update(host)
    }

    /// L'hôte a changé les droits de l'invité (« Vous pouvez jouer à la
    /// manette. ») ; au départ (`previous` nil), le rôle seulement. Le clavier
    /// et la souris retirés : le clavier de l'écran se replie ; les manettes
    /// retirées : la manette à l'écran se retire.
    private func shareRoleChanged(_ role: ShareRole, previous: ShareRole?) {
        // Une session finie, ou qui se ferme, n'écrit plus rien (réglages, avis).
        switch phase {
        case .ending, .ended, .failed: return
        default: break
        }
        shareRole = role
        // Sans le clavier et la souris, la souris de l'iPad n'est plus capturée (souris des jeux).
        streamView.mouseMode = role.keyboardAndMouse ? host.settings.mouse.streamMode : .desktop
        if !role.keyboardAndMouse, keyboardShown { toggleKeyboard() }
        // Sans les manettes, la manette à l'écran se retire (elle n'envoie plus rien).
        if !role.pads, screenPad { toggleScreenPad() }
        if let text = ShareText.roleNotice(role, previous: previous) { show(notice: text, for: 4) }
    }

    /// « Qualité de l'image » dans la barre : le cœur relance l'image de
    /// l'invité seulement (jamais celle de l'hôte) ; la qualité est retenue
    /// pour la fois suivante. La qualité reçue est inconnue jusqu'à
    /// `on_share_image` : la barre dit d'ici là la qualité choisie.
    func setShareQuality(_ quality: ShareQuality) {
        guard isGuest, quality != shareQuality else { return }
        switch phase {
        case .ending, .ended, .failed: return
        default: break
        }
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
        guard isGuest else { return }
        switch phase {
        case .ending, .ended, .failed: return
        default: break
        }
        shareImage = image
        if let notice, !notice.isEmpty { show(notice: notice, for: 4) }
    }

    /// Les avis du cœur, en capsules.
    private func notice(_ n: SessionEngine.Notice) {
        switch n {
        case .relaunched:
            show(notice: String(localized: "L'image a été relancée"), for: 4)
        case let .modeChanged(width, height, back):
            var text = back ? String(localized: "L'ordinateur est revenu en \(width) × \(height)") : String(localized: "L'ordinateur est passé en \(width) × \(height)")
            if let offer = modeRestoreOffer { text += String(localized: " · « \(offer) » dans la barre") }
            show(notice: text)
        case let .fallback(level, mbps):
            fallbackNote = level == 1 ? String(localized: "L'image ne passait pas : débit ramené à \(mbps) Mbit/s pour cette session.") : String(localized: "L'image ne passait pas : 4:2:0 pour cette session.")
            DispatchQueue.main.asyncAfter(deadline: .now() + 6) { [weak self] in self?.fallbackNote = nil }
        case let .adapting(width, height):
            show(notice: String(localized: "Adaptation de l'image à \(width) × \(height)…"), for: 3)
        case let .adapted(width, height):
            show(notice: String(localized: "Image adaptée à \(width) × \(height)"), for: 4)
        case let .adaptFailed(message):
            show(notice: String(localized: "L'écran de l'ordinateur n'a pas pu être adapté : \(message)"), for: 6)
        case let .restoreOffer(mode):
            modeRestoreOffer = mode.map { String(localized: "Remettre \($0.width) × \($0.height)") }
        case .stationRestarting:
            stationRestarting = true
        }
    }

    /// Capsule en haut de l'image pendant quelques secondes (`opensShare` : la choisir
    /// ouvre le panneau du partage).
    func show(notice text: String, for seconds: TimeInterval = 5, opensShare: Bool = false, opensLend: Bool = false) {
        withAnimation(.easeOut(duration: 0.3)) {
            notice = text
            noticeOpensShare = opensShare
            noticeOpensLend = opensLend
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { [weak self] in
            guard self?.notice == text else { return }
            withAnimation(.easeIn(duration: 0.3)) {
                self?.notice = nil
                self?.noticeOpensShare = false
                self?.noticeOpensLend = false
            }
        }
    }

    // MARK: - Prêt de l'ordinateur

    /// Une demande d'emprunt pendant la session : « {prénom} veut emprunter
    /// l'ordinateur. » (8 s ; la choisir ouvre le panneau) ; un créneau qui
    /// commence : « Créneau de {prénom} : l'ordinateur lui est prêté dans 2 min. »
    private func lendRequestArrived(_ request: LendRequest) {
        switch phase {
        case .ending, .ended, .failed: return
        default: break
        }
        if request.slot {
            show(notice: LendText.slotStarts(request.name), for: 8)
        } else if !lendSheet {
            show(notice: LendText.wantsToBorrow(request.name), for: 8, opensLend: true)
        }
    }

    /// Le préavis de 2 minutes, chez l'ami : la capsule reste 10 s, le temps
    /// restant de la barre suit la fin du préavis.
    private func lendNoticed(_ notice: LendNotice, text: String) {
        guard isBorrower else { return }
        switch phase {
        case .ending, .ended, .failed: return
        default: break
        }
        lendNotice = notice
        if !text.isEmpty { show(notice: text, for: 10) }
    }

    /// La fin par le prêt, dite juste avant `.ending` : rendu, la fin est
    /// voulue ; sinon son texte reste à l'écran.
    private func lendEnded(_ end: LendEnd, text: String) {
        guard isBorrower else { return }
        if end == .left {
            givingBack = true
        } else {
            lendEndText = text.isEmpty ? nil : text
        }
    }

    /// « Rendre l'ordinateur » : la confirmation (« Rendre l'ordinateur à {hôte} ? ») d'abord.
    func askGiveBack() {
        guard isBorrower, phase == .streaming || phase == .starting else { return }
        giveBackAsked = true
    }

    /// Confirmé : le prêt finit, puis la session.
    func giveBack() {
        guard isBorrower else { return }
        givingBack = true
        #if DEBUG
        if engine == nil {
            end()
            return
        }
        #endif
        engine?.giveBack()
    }

    /// « Revenir » (carte de fin) : le même emprunt, dans une nouvelle session.
    func comeBack() {
        guard let borrowSource else { return }
        finish()
        onComeBack?(borrowSource)
    }

    /// Une demande vient d'arriver pendant la session : l'avis bref « {prénom} demande
    /// à rejoindre. » (6 s), qui ouvre le panneau du partage quand on le choisit.
    private func requestArrived(_ request: ShareGuest) {
        switch phase {
        case .ending, .ended, .failed: return
        default: break
        }
        guard !shareSheet else { return }
        show(notice: ShareText.requestNotice(request.name), for: 6, opensShare: true)
    }

    // MARK: - Barre

    func setBar(_ open: Bool) {
        guard phase == .streaming || !open else { return }
        // Pupitre (image sur l'écran externe) : la barre reste.
        if !open, external, phase == .streaming { return }
        if !open, padMode {
            padMode = false
            GamepadBridge.shared.setCaptured(false)
        }
        guard open != barOpen else { return }
        withAnimation(.spring(response: 0.32, dampingFraction: 0.86)) { barOpen = open }
        // Pupitre : tout l'écran reste le pavé tactile, barre ouverte ou non.
        streamView.touchesSuspended = open && !external
    }

    func toggleBar() { setBar(!barOpen) }

    /// L1 + R1 + Start + Select : la barre s'ouvre et la manette la pilote, ou
    /// se ferme et la manette revient à l'ordinateur.
    private func gamepadCombo() {
        guard phase == .streaming else { return }
        if padMode {
            setBar(false)
        } else {
            withAnimation(.easeOut(duration: 0.2)) { padMode = true }
            GamepadBridge.shared.setCaptured(true)
            setBar(true)
        }
    }

    /// Profil changé dans la barre : le cœur change le débit en direct, ou
    /// relance l'image pour le 4:4:4, le format ou le transport.
    func select(_ profile: Profile) {
        // Un invité ou un ami ne change rien chez l'hôte : ni profil, ni réglages.
        guard !isVisitor, profile != host.settings.profile else { return }
        host.settings.apply(profile)
        store.update(host)
        engine?.setSettings(engineSettings)
    }

    /// Souris : Auto (jeu en plein écran), Bureau, Jeu.
    func setMouse(_ choice: MouseChoice) {
        host.settings.mouse = choice
        store.update(host)
        streamView.mouseMode = choice.streamMode
    }

    // MARK: - Manette à l'écran

    /// Bouton de la barre : la manette s'affiche (et se branche sur
    /// l'ordinateur) ou se retire (et se débranche) ; retenu pour cet ordinateur.
    func toggleScreenPad() {
        host.settings.screenPad.toggle()
        store.update(host)
        showScreenPad(host.settings.screenPad)
    }

    private func showScreenPad(_ on: Bool) {
        screenPad = on
        streamView.touchAsMouse = !on
        GamepadBridge.shared.setScreenPad(on, name: ScreenPadText.name)
    }

    /// Le petit bouton de la manette : Complète ou Simple, retenu pour cet
    /// ordinateur. La manette reste branchée, à la même place.
    func toggleScreenPadLayout() {
        host.settings.screenPadLayout = host.settings.screenPadLayout == .full ? .simple : .full
        store.update(host)
    }

    /// Pavé tactile ou toucher direct.
    func setDirectTouch(_ on: Bool) {
        prefs.directTouch = on
        streamView.directTouch = on
    }

    var directTouch: Bool { prefs.directTouch }

    /// Sans écran virtuel : afficher un autre écran de l'ordinateur.
    var canPickScreen: Bool { !virtualDisplay && screens.count > 1 }

    func showScreen(_ id: UInt32) {
        guard id != shownDisplay else { return }
        engine?.pickDisplay(id)
    }

    /// « Remettre » : Admin remet le mode demandé au début de la session.
    func restoreMode() {
        modeRestoreOffer = nil
        engine?.restoreMode()
    }

    // MARK: - Clavier et touches

    func toggleKeyboard() {
        keyboardShown.toggle()
        if keyboardShown {
            setBar(false)
            streamView.showKeyboard()
        } else {
            streamView.hideKeyboard()
        }
    }

    func keyboardHidden() { keyboardShown = false }

    func tap(_ usage: UInt16) { streamView.tap(usage: usage) }
    func toggleSticky(_ usage: UInt16) { streamView.toggleSticky(usage) }
    func chord(_ modifiers: [UInt16], _ usage: UInt16) { streamView.chord(modifiers, usage) }

    var sessionMinutes: Int { Int(Date().timeIntervalSince(startedAt) / 60) }

    #if DEBUG
    enum Demo { case connecting, restarting, image, bar }

    /// Démonstration (simulateur), sans ordinateur : l'écran de connexion
    /// arrêté à son étape « Connexion » (ou au redémarrage de la Station,
    /// dernier recours), ou le passage puis l'image (une mire) seule, ou avec
    /// la barre ouverte.
    func demo(_ what: Demo) {
        phase = .preparing
        // KyberNow Share : le partage et le prêt se jouent ici, sans Station.
        share.demo = true
        lend.demo = true
        configureView()
        streamView.startTestPattern()
        guard what != .connecting, what != .restarting else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { [weak self] in
                self?.phase = .connecting
                if what == .restarting { self?.stationRestarting = true }
            }
            return
        }
        externalWatch = ExternalDisplay.shared.$scene.sink { [weak self] scene in
            DispatchQueue.main.async { self?.useExternal(scene != nil, adapt: true) }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in self?.phase = .starting }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) { [weak self] in
            guard let self else { return }
            withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.2)) {
                self.revealedAt = Date()
                self.phase = .streaming
            }
            self.startedAt = Date().addingTimeInterval(-12 * 60)
            self.modeLabel = "2622 × 1206 · 120 Hz"
            for i in 0..<30 {
                var s = VideoStats()
                s.latencyMs = 8.4 + Double(i % 7) * 0.4
                s.fps = 120
                s.mbps = 46
                s.format = "4:2:0 8 bits"
                s.width = 2622
                s.height = 1206
                self.received(s)
            }
            if what == .bar { DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { self.setBar(true) } }
            if UserDefaults.standard.bool(forKey: "demoKeyboard") {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { self.toggleKeyboard() }
            }
            // `-demoShare YES` : le partage ouvert, quatre invités, le panneau de l'hôte.
            if UserDefaults.standard.bool(forKey: "demoShare") {
                Task {
                    await self.share.openIfNeeded()
                    // Léa et Sam reçoivent une image à part (leur qualité se voit) ; Hugo suit l'hôte ; Zoé : rien de su.
                    self.share.demoAddGuest(name: "Léa", app: "ios", role: .player, image: ShareImage.demo("1080p60", 1920, 1080, 60, follows: false))
                    self.share.demoAddGuest(name: "Hugo", app: "tvos", image: ShareImage.demo("host", 2560, 1440, 120, follows: true))
                    self.share.demoAddGuest(name: "Sam", app: "web", role: .playerKbm, image: ShareImage.demo("720p30", 1280, 720, 30, follows: false))
                    self.share.demoAddGuest(name: "Zoé", app: "mac", role: .kbm, image: ShareImage.demo("auto"))
                    self.shareSheet = true
                }
            }
            // `-demoLend YES` : le panneau « Prêter l'ordinateur… » ouvert ; `-demoLendRequest Léa` :
            // une demande d'emprunt arrive (capsule, puis panneau) ; `-demoLendCode YES` : un code créé.
            if UserDefaults.standard.bool(forKey: "demoLend") || UserDefaults.standard.string(forKey: "demoLendRequest") != nil {
                Task {
                    if UserDefaults.standard.bool(forKey: "demoLendCode") { await self.lend.createCode() }
                    if let name = UserDefaults.standard.string(forKey: "demoLendRequest") {
                        self.demoLendRequest(name: name)
                    }
                    if UserDefaults.standard.bool(forKey: "demoLend") { self.lendSheet = true }
                }
            }
            // `-demoRelay YES` : la session passe par le relais (puce « Par le relais », ligne « Chemin »).
            if UserDefaults.standard.bool(forKey: "demoRelay") {
                self.path = RouteInfo(transport: .relay, relay: Relay.defaultServer, rttClient: 24, rttStation: 11)
            }
            // `-demoRequests 1|2` : le partage de l'hôte, relais joint, avec ce nombre de demandes en attente
            // (`-demoShareRelay unreachable` : relais réglé mais pas joint), le panneau ouvert.
            let requests = UserDefaults.standard.integer(forKey: "demoRequests")
            if requests > 0 || UserDefaults.standard.string(forKey: "demoShareRelay") != nil {
                Task {
                    await self.share.openIfNeeded()
                    self.share.demoSetRelay(ShareState.Relay(wire: UserDefaults.standard.string(forKey: "demoShareRelay") ?? "joined"))
                    if requests > 0 { self.share.demoAddGuest(name: "Léa", app: "ios", pending: true, viaRelay: true) }
                    if requests > 1 { self.share.demoAddGuest(name: "Hugo", app: "tvos", pending: true, viaRelay: true) }
                    self.share.demoAddGuest(name: "Sam", app: "android", role: .player, viaRelay: true)
                    self.share.demoAddGuest(name: "Zoé", app: "mac", role: .kbm)
                    if UserDefaults.standard.bool(forKey: "demoSheetClosed") == false { self.shareSheet = true }
                }
            }
            // `-demoRole spectator|player|kbm|player_kbm` : séance d'invité (`-screen guest`), les droits d'abord ;
            // `-demoRoleThen spectator|player|kbm|player_kbm|end-stopped|end-removed` : ce que l'hôte fait 6 s plus tard.
            if let then = UserDefaults.standard.string(forKey: "demoRoleThen") {
                DispatchQueue.main.asyncAfter(deadline: .now() + 6) {
                    switch then {
                    case "spectator": self.demoRoleChanged(.spectator)
                    case "player": self.demoRoleChanged(.player)
                    case "kbm": self.demoRoleChanged(.kbm)
                    case "player_kbm": self.demoRoleChanged(.playerKbm)
                    case "end-stopped": self.demoShareEnded(removed: false)
                    case "end-removed": self.demoShareEnded(removed: true)
                    case "lag": self.demoImageNotice(limit: false)
                    case "limit": self.demoImageNotice(limit: true)
                    default: break
                    }
                }
            }
        }
    }

    /// Démonstration : la session d'un invité (sans Station), au rôle demandé.
    func demoGuest(role: ShareRole) {
        shareRole = role
        // La qualité reçue dans la barre : une image à part en 1080p · 60 Hz.
        shareImage = ShareImage.demo("1080p60", 1920, 1080, 60, follows: false)
        demo(.bar)
    }

    /// Démonstration : l'avis du cœur sur l'image (`-demoRoleThen lag|limit`).
    func demoImageNotice(limit: Bool) {
        guard isGuest else { return }
        if limit {
            shareImageChanged(ShareImage.demo("host", 1920, 1080, 60, follows: false), notice: "L'ordinateur ne peut pas faire une image de plus : vous recevez 1080p · 60 Hz.")
        } else {
            shareImageChanged(ShareImage.demo("1080p60", 1920, 1080, 30, follows: false), notice: "Image allégée pour suivre.")
        }
    }

    /// Démonstration : l'hôte change le rôle de l'invité.
    func demoRoleChanged(_ role: ShareRole) {
        guard isGuest else { return }
        let previous = shareRole
        shareRoleChanged(role, previous: previous)
    }

    /// Démonstration : la session d'emprunt (sans Station) : `waiting`
    /// (« En attente de {hôte}… »), sinon en image, le prêt jusqu'à `until` ;
    /// `-demoBorrowThen notice|taken_back|time_up|slot_end|lost` : ce qui arrive 5 s plus tard.
    func demoBorrow(waiting: Bool, until: Date?) {
        let host = borrowSource?.hostHint.nonEmpty ?? "Matthieu"
        if waiting {
            phase = .preparing
            configureView()
            borrowFlow?.demo(nil, waiting: host)
            return
        }
        loan = LendLoan(demoComputer: "PC-SALON", host: host, until: until)
        demo(.bar)
        if let then = UserDefaults.standard.string(forKey: "demoBorrowThen") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 5) { self.demoLend(then) }
        }
    }

    /// Démonstration : le préavis, ou la fin du prêt (`lost` : réseau perdu, « Revenir »).
    func demoLend(_ moment: String) {
        guard isBorrower, phase == .streaming else { return }
        let host = lendHost
        switch moment {
        case "notice":
            lendNoticed(LendNotice(reason: .takenBack, at: Date().addingTimeInterval(120)),
                        text: LendNotice(reason: .takenBack, at: Date()).text(name: host))
        case "lost":
            fail(String(localized: "l'ordinateur prêté ne répond plus"))
        default:
            let text = moment == "time_up" ? String(localized: "Le prêt est terminé.")
                : moment == "slot_end" ? String(localized: "Votre créneau est terminé.") : String(localized: "\(host) a repris l'ordinateur.")
            lendEndText = text
            fail(text)
        }
    }

    /// Démonstration : une demande d'emprunt arrive dans la session de l'hôte.
    func demoLendRequest(name: String, slot: Bool = false) {
        lend.demo = true
        var view = lend.view ?? LendView()
        view.requests.append(LendRequest(id: UUID().uuidString, name: name, app: "ios", viaRelay: true, since: Date(), slot: slot))
        lend.demoSet(view)
    }

    /// Démonstration : l'hôte arrête le partage ou retire l'invité.
    func demoShareEnded(removed: Bool) {
        guard isGuest, phase == .streaming else { return }
        let host = guest?.host ?? "Léa"
        shareEndText = removed ? String(localized: "\(host) vous a retiré du partage.") : String(localized: "\(host) a arrêté le partage.")
        fail(shareEndText ?? "")
    }
    #endif

    // MARK: - Fin

    /// Fin de session : le cœur rend la main à Admin (écrans et son de
    /// l'ordinateur rétablis, latence donnée à ses appareils).
    func end() {
        guard phase != .ending && phase != .ended else { return }
        // « Annuler » pendant la préparation ou l'attente de l'hôte : la demande est retirée.
        if loan == nil { borrowFlow?.cancel() }
        GamepadBridge.shared.onMenuRequest = nil
        streamView.hideKeyboard()
        // Un invité ne laisse rien sur cet appareil : ni dernière image, ni statistiques.
        if let last = engine?.snapshot(maxWidth: 1600), !isVisitor { store.imagery.saveLast(last, for: host) }
        // Statistiques : la dernière session d'au moins 3 s, ce qu'elle a mesuré et transmis.
        if let revealedAt, !isVisitor {
            let codec = engineSettings.codec
            let mbps = host.settings.bitrate(width: streamMode.width, height: streamMode.height, refresh: streamMode.refresh)
            host.recordSession(start: revealedAt, latency: engine?.sessionLatency, width: streamMode.width, height: streamMode.height,
                               refresh: streamMode.refresh, codec: codec, mbps: mbps, route: path.transport)
        }
        store.update(host)
        withAnimation(.easeInOut(duration: 0.4)) {
            phase = .ending
            barOpen = false
        }
        leaveExternal()
        streamView.detach()
        engine?.stop()
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in self?.finish() }
    }

    /// Fin de session : l'écran externe retrouve son image d'attente.
    private func leaveExternal() {
        externalWatch = nil
        guard external else { return }
        external = false
        streamView.showImage(in: nil)
        ExternalDisplay.shared.showingImage(false)
    }

    func finish() {
        guard phase != .ended else { return }
        phase = .ended
        if !isVisitor { store.setSessionViaRelay(host, false) }
        // L'emprunt pas encore abouti : la demande est retirée chez l'hôte.
        borrowFlow?.cancel()
        onFinish(self)
    }

    private func fail(_ message: String) {
        KyberLog.app.error("session : \(message, privacy: .public)")
        GamepadBridge.shared.onMenuRequest = nil
        leaveExternal()
        streamView.detach()
        engine?.stop()
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        withAnimation(Theme.easeOut) {
            barOpen = false
            phase = .failed(message)
        }
    }

    func dismissFailure() { finish() }
}
