// Ce que la session est sur l'iPhone, l'iPad et l'Apple TV, sans son
// interface (audit IOS-21, TVOS-20 à 22) : l'état et les réactions communes
// aux deux apps, au lieu de deux copies qui divergeaient (bandeaux de
// PyroWave, nettoyages, visiteurs). Chaque app garde dans son
// `SessionController` la présentation (barre et clavier sur iPhone et iPad,
// menu et téléviseur sur l'Apple TV) et ce qui tient à l'appareil (écran
// externe, fréquence du téléviseur, micro). Compilé par les deux apps
// (apps/ios/project.yml le reprend, comme CloudSync.swift).
//
// Le déroulé lui-même (préparation par Admin, connexion à Kyber, relances,
// reconnexions, repli, fin vérifiée) est dans le cœur Rust, par
// `SessionEngine` (docs/coeur-apple.md).

import AVFoundation
import Combine
import KyberKit
import SwiftUI

@MainActor
class SessionModel: ObservableObject {
    enum Phase: Equatable {
        case preparing, connecting, starting, streaming
        case reconnecting(Int)
        case ending
        case ended
        case failed(String)

        var stage: SessionStep {
            switch self {
            case .preparing: .preparing
            case .connecting: .connecting
            case .starting: .starting
            case .streaming: .streaming
            case .reconnecting: .reconnecting
            case .ending: .ending
            case .ended: .ended
            case .failed: .failed
            }
        }
    }

    /// La veille (et l'économiseur de l'Apple TV) est retenue tant que la
    /// session vit (`SessionRules.keepsAwake`).
    @Published var phase: Phase = .preparing {
        didSet { SleepGuard.hold(self, SessionRules.keepsAwake(phase.stage)) }
    }
    @Published var host: Host
    /// Instant où l'image est apparue (le passage s'ouvre).
    @Published var revealedAt: Date?
    /// Dernier recours du cœur : l'image ne venait pas, il redémarre la
    /// Station et attend son retour (« La Station redémarre… »), jusqu'à
    /// l'état suivant.
    @Published var stationRestarting = false
    @Published var route = AudioRoute.current
    /// Par où passe la session (docs/relais.md) : direct sur le réseau, ou par
    /// le relais hors de chez soi (puce « Par le relais », « Chemin » des
    /// statistiques). Donné par le cœur, au départ puis quand il change.
    @Published var path = RouteInfo.direct
    @Published var modeLabel = ""
    @Published var startedAt = Date()
    /// Capsule d'information en haut de l'image (mode changé, image relancée…).
    @Published var notice: String?
    @Published var noticeIcon: String?
    /// La capsule ouvre le panneau du partage (une demande pour rejoindre),
    /// celui du prêt (une demande d'emprunt), ou porte « Ouvrir les Réglages »
    /// (caméra ou micro refusés). Au toucher seulement (pas à la télécommande).
    @Published var noticeOpensShare = false
    @Published var noticeOpensLend = false
    @Published var noticeOpensSettings = false
    /// Le repli de l'image (« débit ramené », « 4:2:0 »), 6 s.
    @Published var fallbackNote: String?
    /// Profil Bureau, écran de l'ordinateur passé à un autre mode : « Remettre
    /// 2560 × 1440 » (le cœur le propose et le retire).
    @Published var modeRestoreOffer: String?

    // MARK: Caméra de l'appareil (docs/webcam.md)

    /// L'ordinateur propose la caméra.
    @Published var cameraOffered = false
    @Published var cameraState: CameraState = .off
    /// La ligne « Caméra » des statistiques (nil : éteinte).
    var cameraStatsLine: String? { engine?.cameraStatsLine }

    let streamView: StreamView
    let store: HostStore
    let prefs = Preferences.shared
    var engine: SessionEngine?
    /// Le cœur est à l'image (pour l'apparition retardée de la première).
    var engineStreaming = false
    /// Mode de l'écran de la session (demandé, puis celui obtenu de
    /// l'ordinateur) : gardé avec la dernière session.
    var streamMode = (width: 0, height: 0, refresh: 0)
    /// Les manettes branchées, pour l'avis des manettes indisponibles ; déjà
    /// dit dans cette session.
    var padsWatch: AnyCancellable?
    private var padsUnavailableShown = false
    private var fallbackNoteClose: DispatchWorkItem?
    static let revealDelay: TimeInterval = 0.9

    // MARK: KyberNow Share

    /// La session d'un invité : ce que `Share.join` a rendu ; nil pour une
    /// session d'hôte. Pour qui regarde un ordinateur du compte, posé au départ.
    @Published var guest: ShareFound?
    /// « Regarder » un ordinateur du compte : l'ordinateur regardé ; la session
    /// est ensuite celle d'un invité spectateur.
    let watchHost: Host?
    var isWatching: Bool { watchHost != nil }
    var isGuest: Bool { guest != nil || watchHost != nil }
    /// Les droits que l'hôte donne à l'invité (`on_share_role`).
    @Published var shareRole: ShareRole = .spectator
    /// La « Qualité de l'image » de l'invité (celle demandée) et l'image qu'il
    /// reçoit (`on_share_image`) ; l'interface montre `shareImageLabel`.
    @Published var shareQuality: ShareQuality = .auto
    @Published var shareImage: ShareImage?
    /// La qualité reçue, ou la qualité choisie tant que la taille n'est pas sue.
    var shareImageLabel: String { GuestQuality.barLabel(image: shareImage, quality: shareQuality) }
    /// La raison d'une fin par le partage (« {hôte} a arrêté le partage. »).
    @Published var shareEndText: String?
    /// Le partage de cette session d'hôte (« Partager »).
    let share: ShareHostModel
    /// « Partager » avec cet ordinateur : disponible, ou inactif avec la raison.
    var shareAvailability: ShareAvailability { share.availability }

    // MARK: Prêt de l'ordinateur

    /// L'emprunt d'un ami (code de prêt, compte d'ami) ; nil pour une autre session.
    let borrowSource: BorrowSource?
    var isBorrower: Bool { borrowSource != nil }
    /// Un visiteur (invité, ami) : rien n'est gardé ni changé chez l'hôte.
    var isVisitor: Bool { isGuest || isBorrower }
    /// L'emprunt en cours, jusqu'à l'accord (« En attente de {hôte}… »).
    let borrowFlow: BorrowFlow?
    var borrowWatch: AnyCancellable?
    /// Le prêt obtenu (Station, écran virtuel, fin prévue).
    @Published var loan: LendLoan?
    /// Le préavis de 2 minutes, chez l'ami.
    @Published var lendNotice: LendNotice?
    /// La raison d'une fin par le prêt (« {hôte} a repris l'ordinateur. »).
    @Published var lendEndText: String?
    /// « Rendre l'ordinateur à {hôte} ? » est posé.
    @Published var giveBackAsked = false
    /// L'ami rend l'ordinateur : la fin est voulue.
    var givingBack = false
    /// {hôte} du prêt : celui du prêt obtenu, sinon celui de la demande.
    var lendHost: String { loan?.host ?? borrowFlow?.host.nonEmpty ?? borrowSource?.hostHint.nonEmpty ?? host.name }
    /// « Revenir » est possible : un emprunt fini sans que le prêt le dise.
    var canComeBack: Bool { isBorrower && loan != nil && lendEndText == nil && !givingBack }
    /// « Revenir » : l'app ouvre le même emprunt dans une nouvelle session.
    var onComeBack: ((BorrowSource) -> Void)?
    /// Le prêt de cet ordinateur, depuis la session de l'hôte (« Prêter l'ordinateur… »).
    let lend: LendHostModel

    // MARK: Enregistrement de la session

    /// L'enregistrement (docs/enregistrement.md), hôte comme invité ; nil tant
    /// que la Station n'a rien dit (Station qui n'enregistre pas) et pour un
    /// ami qui emprunte.
    @Published var recording: RecordingState?
    /// Une demande de lancement ou d'arrêt est partie (pas de double appui).
    private var recordingBusy = false
    /// L'enregistrement se montre : la Station enregistre et l'a dit.
    var recordingShown: Bool { recording?.available == true && !isBorrower }
    /// L'hôte enregistre en ce moment (pastille rouge).
    var recordingActive: Bool { recordingShown && recording?.active == true }

    // MARK: Création

    /// Ce qu'est la session : celle de l'ordinateur, d'un invité, de qui
    /// regarde un ordinateur du compte, d'un ami qui emprunte.
    enum Kind {
        case computer
        case guest(ShareFound)
        case watch(Host)
        case borrow(BorrowSource)
    }

    init(_ kind: Kind, host computer: Host? = nil, store: HostStore, streamView: StreamView) {
        self.store = store
        self.streamView = streamView
        let first: Host
        switch kind {
        case .computer:
            first = computer ?? Host(name: "", address: "", consolePort: 0, username: "")
            guest = nil
            watchHost = nil
            borrowSource = nil
            borrowFlow = nil
        case let .guest(found):
            first = Host.visitor(guest: found)
            guest = found
            watchHost = nil
            borrowSource = nil
            borrowFlow = nil
            shareRole = found.role
            shareQuality = found.quality
        case let .watch(computer):
            first = Host.visitor(watch: computer)
            guest = nil
            watchHost = computer
            borrowSource = nil
            borrowFlow = nil
            shareQuality = GuestQuality.proposed
        case let .borrow(source):
            first = Host.visitor(borrow: source)
            guest = nil
            watchHost = nil
            borrowSource = source
            borrowFlow = BorrowFlow(source: source)
        }
        host = first
        if case .computer = kind {
            let id = first.id
            share = ShareHostModel(console: nil) { [store] in ShareHostModel.availability(of: id, in: store) }
            lend = LendHostModel(console: nil) { [store] in store.lendAvailability(of: id) }
        } else {
            share = ShareHostModel(console: nil) { .available }
            lend = LendHostModel(console: nil) { .available }
        }
    }

    // MARK: Ce que chaque app fait à sa façon

    /// Fin voulue (« Se déconnecter »), échec, et sortie de la session : propres à chaque app.
    func end() { preconditionFailure("SessionController.end") }
    func fail(_ message: String) { preconditionFailure("SessionController.fail") }
    func finish() { preconditionFailure("SessionController.finish") }
    /// Le panneau du partage, ou celui du prêt, est ouvert : une demande s'y voit déjà.
    var sharePanelOpen: Bool { false }
    var lendPanelOpen: Bool { false }
    /// Durée des fondus des capsules (s) et de l'apparition de la première image.
    var fadeDuration: Double { 0.3 }
    var revealDuration: Double { 1.2 }
    /// Où se trouvent « Remettre » et les commandes : « dans la barre », « dans le menu ».
    func restoreOfferHint(_ offer: String) -> String { "" }
    /// L'adaptation de l'écran de l'ordinateur se dit (l'Apple TV ne change pas d'écran en session).
    var tellsAdaptation: Bool { true }
    /// La première image est apparue (après le passage).
    func revealed() {}
    /// Les droits de l'invité ont changé : le clavier, la souris, la manette à l'écran suivent.
    func rightsChanged(_ role: ShareRole) {}
    /// L'ordinateur propose la caméra (liste de l'appareil à relire).
    func cameraOfferChanged(_ offered: Bool) {}
    /// Le test du profil Auto s'ouvrira à la fin de la session.
    func openAutoTestAfterSession() {}

    // MARK: Session

    /// La session tient encore (ni finie, ni en échec, ni qui se ferme).
    var isLive: Bool {
        switch phase {
        case .ending, .ended, .failed: return false
        default: return true
        }
    }

    /// Le titre de la session : le nom de l'ordinateur, « Invité sur {nom} »,
    /// « Regarder {nom} », ou « Ordinateur de {hôte} ».
    var title: String {
        if isBorrower { return LendText.computerOf(lendHost) }
        if let watchHost { return Share.watchTitle(watchHost.name) }
        return guest.map { ShareText.guestOn($0.computer) } ?? host.name
    }

    /// « Quitter le partage », ou « Arrêter de regarder » pour qui regarde.
    var leaveLabel: String { isWatching ? Share.watchWord(.stop) : ShareText.leave }

    /// Regarder : « Écran » (plusieurs écrans qu'on peut changer).
    var watchScreens: [ShareWatchScreen] {
        guard let watch = guest?.watch, watch.menu else { return [] }
        return watch.screens
    }

    /// Regarder : l'écran regardé.
    var watchDisplay: UInt32 {
        let shown = engine?.watchDisplay ?? 0
        return shown != 0 ? shown : guest?.watch?.display ?? 0
    }

    /// Regarder : un autre écran ; la session se reconnecte dessus.
    func watchScreen(_ id: UInt32) {
        guard isWatching, phase == .streaming || phase == .starting, id != watchDisplay else { return }
        engine?.watchScreen(id)
        objectWillChange.send()
    }

    /// Les réglages de l'image, pour le cœur (`ConnectionSettings.engineSettings`).
    var engineSettings: SessionEngine.Settings {
        host.settings.engineSettings(codecOverride: codecOverride)
    }

    /// Le format de l'image reçue : celui de son image (un invité), à défaut
    /// celui de l'hôte, sinon celui des réglages.
    var imageCodec: VideoCodec { receivedCodec ?? shareImage?.codec ?? guest?.codec ?? host.settings.codec }

    // MARK: - Départ

    /// Le départ commun : le mot de passe de l'ordinateur ; pour qui regarde,
    /// l'entrée dans le partage de l'ordinateur d'abord (« Préparation de
    /// l'écran ») ; pour un ami, l'emprunt d'abord (« Préparation de
    /// l'ordinateur », « En attente de {hôte}… »), à l'écran de cet appareil.
    /// Puis `launch(password)`, la session du cœur.
    func begin(shareScreen: @autoclosure () -> ShareScreen, lendScreen: @autoclosure () -> LendScreen, launch: @escaping (String) -> Void) {
        // Un invité n'a pas de compte : le code a suffi (un ami : le prêt).
        guard let password = isVisitor ? "" : store.password(for: host) else {
            fail(String(localized: "mot de passe introuvable dans le Trousseau"))
            return
        }
        if let watchHost, guest == nil {
            phase = .preparing
            let quality = shareQuality
            let screen = shareScreen()
            Task { [weak self] in
                guard let accounts = self?.store.accounts else { return }
                do {
                    let found = try await accounts.watch(watchHost, quality: quality, screen: screen)
                    guard let self, self.phase == .preparing else {
                        Share.leave(found)
                        return
                    }
                    self.guest = found
                    self.shareRole = found.role
                    launch(password)
                } catch {
                    guard let self, self.phase == .preparing else { return }
                    let text = ShareJoinModel.text(error)
                    self.shareEndText = text
                    self.fail(text)
                }
            }
            return
        }
        if let flow = borrowFlow, loan == nil {
            phase = .preparing
            borrowWatch = flow.objectWillChange.sink { [weak self] _ in self?.objectWillChange.send() }
            flow.onReady = { [weak self] loan in
                guard let self, self.phase == .preparing else {
                    Lend.leave(loan)
                    return
                }
                self.loan = loan
                self.host.name = LendText.computerOf(loan.host)
                launch(password)
            }
            flow.onFailed = { [weak self] text in
                guard let self, self.isLive else { return }
                self.lendEndText = text
                self.fail(text)
            }
            flow.start(screen: lendScreen())
            return
        }
        launch(password)
    }

    /// Les rappels du moteur que les deux apps traitent de la même façon ;
    /// chaque app ajoute les siens ensuite (image, HDR, son, manettes).
    func wire(_ engine: SessionEngine) {
        engine.onPhase = { [weak self] in self?.phaseChanged($0) }
        engine.onNotice = { [weak self] in self?.notice($0) }
        engine.onCameraOffer = { [weak self] offered in
            guard let self else { return }
            KyberLog.app.notice("caméra : \(offered ? "proposée" : "plus proposée", privacy: .public) par l'ordinateur")
            cameraOffered = offered
            if offered, host.cameraOffered != true, !isVisitor {
                host.cameraOffered = true
                store.update(host)
            }
            cameraOfferChanged(offered)
        }
        engine.onCameraState = { [weak self] state in
            KyberLog.app.notice("caméra : \(String(describing: state), privacy: .public)")
            self?.cameraState = state
        }
        engine.onCameraNotice = { [weak self] text in
            KyberLog.app.notice("caméra : avis « \(text, privacy: .public) »")
            // Accès refusé : l'avis mène aux Réglages (docs/link-ui.md, « Caméra de l'appareil »).
            let denied = [AVAuthorizationStatus.denied, .restricted].contains(AVCaptureDevice.authorizationStatus(for: .video))
            self?.show(notice: text, for: denied ? 6 : 4, opensSettings: denied)
        }
        engine.onLowered = { [weak self] mode, text in self?.resolutionLowered(mode, text) }
        engine.onPyroWaveStrained = { [weak self] in self?.pyrowaveStrainedNotice() }
        engine.onPyroWaveFallback = { [weak self] _, text in self?.pyrowaveFallback(text) }
        engine.onEncoderCeiling = { [weak self] mbps in
            guard let self, !self.isVisitor else { return }
            self.host.settings.encoderMax = mbps
            self.store.update(self.host)
        }
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
        // Enregistrement : l'état (hôte et invité) et l'avis bref du cœur.
        engine.onRecording = { [weak self] state, notice in self?.recordingChanged(state, notice: notice) }
        engine.onRoute = { [weak self] in self?.routeChanged($0) }
        self.engine = engine
        if !isVisitor {
            share.setConsole(store.console(for: host))
            lend.setConsole(store.console(for: host))
        }
    }

    /// Les paramètres communs d'une session : celle d'un ami (la Station du
    /// prêt, sur l'écran virtuel qu'elle a fait pour cet appareil), d'un
    /// invité (la Station qui a accepté le code, sans compte ni écran
    /// virtuel), ou de l'ordinateur (hors de chez soi : le réseau d'abord, puis
    /// le relais si l'accès est actif ; un ordinateur d'un compte KyberNow :
    /// le compte et sa Station).
    func params(mode: (width: Int, height: Int, refresh: Int), password: String, channels: Int) -> SessionEngine.Params {
        let settings = host.settings
        var params: SessionEngine.Params
        if let loan {
            params = SessionEngine.Params(borrower: loan, audioChannels: channels, settings: engineSettings)
        } else if let guest {
            params = SessionEngine.Params(guest: guest, audioChannels: channels, settings: engineSettings)
        } else {
            params = SessionEngine.Params(
                host: host.address, consolePort: host.consolePort, username: host.username, password: password,
                consoleFingerprint: host.consoleFingerprint,
                mode: SessionEngine.Mode(width: mode.width, height: mode.height, refresh: mode.refresh),
                audioChannels: channels, virtualDisplay: settings.display != .physical, settings: engineSettings
            )
            params.relay = host.sessionRelay
            params.account = store.sessionAccount(for: host)
        }
        // Par le relais, une fin faute de joindre l'ordinateur le dit avec son
        // nom (l'ordinateur du partage pour un invité).
        params.computerName = host.name
        params.autoReconnect = settings.autoReconnect
        params.gamepadAsXbox360 = settings.gamepad == .xbox360
        return params
    }

    // MARK: - États du cœur

    /// Les états du cœur : l'image n'apparaît qu'après `revealDelay`, seule.
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
            // Fin demandée par Admin : comme une fin voulue.
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
        withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: revealDuration)) {
            revealedAt = Date()
            phase = .streaming
        }
        startedAt = Date()
        if !isVisitor {
            host.lastSession = Date()
            store.update(host)
        }
        revealed()
        padsUnavailableNotice()
    }

    /// « Manettes indisponibles sur cet ordinateur. » (docs/link-ui.md) : une
    /// fois par session, à l'arrivée de l'image si une manette est branchée
    /// (la manette à l'écran comprise), sinon quand la première se branche.
    /// Pas pour un invité ni un ami, qui ne savent pas ce que l'ordinateur crée.
    func padsUnavailableNotice() {
        guard !isVisitor, !host.gamepadsAvailable, !padsUnavailableShown, phase == .streaming,
              !GamepadBridge.shared.pads.isEmpty else { return }
        padsUnavailableShown = true
        show(notice: LinkText.controllersUnavailableNotice, icon: "game-controller")
    }

    /// L'avis des manettes indisponibles a été dit (le branchement d'une
    /// manette ne se dit qu'ensuite).
    var padsUnavailableTold: Bool { padsUnavailableShown }

    /// Les avis du cœur, en capsules.
    private func notice(_ n: SessionEngine.Notice) {
        switch n {
        case .relaunched:
            show(notice: String(localized: "L'image a été relancée"), for: 4)
        case let .modeChanged(width, height, back):
            streamMode = (width, height, streamMode.refresh)
            modeLabel = SessionModel.modeLabel(width, height, streamMode.refresh)
            var text = back ? String(localized: "L'ordinateur est revenu en \(width) × \(height)") : String(localized: "L'ordinateur est passé en \(width) × \(height)")
            if let offer = modeRestoreOffer { text += restoreOfferHint(offer) }
            show(notice: text)
        case let .fallback(level, mbps):
            fallbackNote = level == 1 ? String(localized: "L'image ne passait pas : débit ramené à \(mbps) Mbit/s pour cette session.") : String(localized: "L'image ne passait pas : 4:2:0 pour cette session.")
            fallbackNoteClose?.cancel()
            let close = DispatchWorkItem { [weak self] in self?.fallbackNote = nil }
            fallbackNoteClose = close
            DispatchQueue.main.asyncAfter(deadline: .now() + 6, execute: close)
        case let .adapting(width, height):
            if tellsAdaptation { show(notice: String(localized: "Adaptation de l'image à \(width) × \(height)…"), for: 3) }
        case let .adapted(width, height):
            if tellsAdaptation { show(notice: String(localized: "Image adaptée à \(width) × \(height)"), for: 4) }
        case let .adaptFailed(message):
            if tellsAdaptation { show(notice: String(localized: "L'écran de l'ordinateur n'a pas pu être adapté : \(message)"), for: 6) }
        case let .restoreOffer(mode):
            modeRestoreOffer = mode.map { String(localized: "Remettre \($0.width) × \($0.height)") }
        case .stationRestarting:
            stationRestarting = true
        case let .hdrRefused(text):
            // Windows a refusé le HDR : une fois par session ; le texte du cœur.
            show(notice: text, for: 6)
        }
    }

    /// « 2560 × 1440 · 120 Hz » : le mode de la session, dans les statistiques.
    static func modeLabel(_ width: Int, _ height: Int, _ refresh: Int) -> String {
        "\(width) × \(height) · \(refresh) Hz"
    }

    /// Capsule en haut de l'image pendant quelques secondes. `opensShare`,
    /// `opensLend` : la choisir ouvre le panneau du partage ou du prêt ;
    /// `opensSettings` : elle porte « Ouvrir les Réglages ». La télécommande
    /// ne choisit pas une capsule : l'Apple TV ne s'en sert pas.
    func show(notice text: String, for seconds: TimeInterval = 5, icon: String? = nil, opensShare: Bool = false, opensLend: Bool = false, opensSettings: Bool = false) {
        let fade = fadeDuration
        withAnimation(.easeOut(duration: fade)) {
            notice = text
            noticeIcon = icon
            noticeOpensShare = opensShare
            noticeOpensLend = opensLend
            noticeOpensSettings = opensSettings
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { [weak self] in
            guard let self, self.notice == text else { return }
            withAnimation(.easeIn(duration: fade)) {
                self.notice = nil
                self.noticeIcon = nil
                self.noticeOpensShare = false
                self.noticeOpensLend = false
                self.noticeOpensSettings = false
            }
        }
    }

    /// « Remettre » : Admin remet le mode demandé au début de la session.
    func restoreMode() {
        modeRestoreOffer = nil
        engine?.restoreMode()
    }

    /// Profil changé en session : le cœur change le débit en direct, ou
    /// relance l'image pour le 4:4:4, le format ou le transport.
    func select(_ profile: Profile) {
        // Un invité ou un ami ne change rien chez l'hôte : ni profil, ni réglages.
        guard !isVisitor, profile != host.settings.profile else { return }
        // Auto sans test gardé : le panneau du test, à la fin de la session.
        if profile == .auto, host.settings.auto == nil {
            runAutoTest()
            return
        }
        host.settings.apply(profile)
        store.update(host)
        // Un profil choisi en session reprend la main sur « Passer en HEVC ».
        codecOverride = nil
        engine?.setSettings(engineSettings)
    }

    // MARK: - Hors de chez soi

    /// Le chemin de la session a changé (au départ, puis toutes les 5 s au plus
    /// s'il change) : la puce « Par le relais » suit. Par le relais, le partage
    /// et le prêt de l'hôte passent aussi par lui (Admin n'est plus joignable
    /// sur le réseau), et l'app ne redémarre ni ne met à jour la Station.
    private func routeChanged(_ info: RouteInfo) {
        guard isLive, info != path else { return }
        let wasRelay = path.viaRelay
        path = info
        if info.viaRelay != wasRelay, !isVisitor {
            share.setConsole(store.console(for: host, route: info.viaRelay ? .relay : .direct))
            lend.setConsole(store.console(for: host, route: info.viaRelay ? .relay : .direct))
            store.setSessionViaRelay(host, info.viaRelay)
        }
    }

    // MARK: - KyberNow Share

    /// L'hôte a changé les droits de l'invité (« Vous pouvez jouer à la
    /// manette. ») ; au départ (`previous` nil), le rôle seulement.
    private func shareRoleChanged(_ role: ShareRole, previous: ShareRole?) {
        // Une session finie, ou qui se ferme, n'écrit plus rien (réglages, avis).
        guard isLive else { return }
        shareRole = role
        rightsChanged(role)
        if let text = ShareText.roleNotice(role, previous: previous) { show(notice: text, for: 4) }
    }

    /// « Qualité de l'image » : le cœur relance l'image de l'invité seulement
    /// (jamais celle de l'hôte) ; la qualité est retenue pour la fois
    /// suivante. La qualité reçue est inconnue jusqu'à `on_share_image` :
    /// d'ici là, la qualité choisie.
    func setShareQuality(_ quality: ShareQuality) {
        guard isGuest, quality != shareQuality, isLive else { return }
        shareQuality = quality
        shareImage = nil
        GuestQuality.remember(quality)
        engine?.setShareQuality(quality)
    }

    /// L'image que reçoit l'invité a changé (au départ, ou après un changement
    /// de qualité, un retard, une limite de l'ordinateur) ; l'avis du cœur
    /// (« Image allégée pour suivre. ») s'affiche en capsule comme les autres.
    func shareImageChanged(_ image: ShareImage, notice: String?) {
        guard isGuest, isLive else { return }
        shareImage = image
        if let notice, !notice.isEmpty { show(notice: notice, for: 4) }
    }

    /// Une demande vient d'arriver pendant la session : l'avis bref « {prénom}
    /// demande à rejoindre. » (6 s), qui ouvre le panneau du partage quand on
    /// le choisit ; rien si le panneau est déjà ouvert (la demande y est).
    private func requestArrived(_ request: ShareGuest) {
        guard isLive, !sharePanelOpen else { return }
        show(notice: ShareText.requestNotice(request.name), for: 6, opensShare: true)
    }

    // MARK: - Prêt de l'ordinateur

    /// Une demande d'emprunt pendant la session : « {prénom} veut emprunter
    /// l'ordinateur. » (8 s ; la choisir ouvre le panneau, rien s'il est
    /// ouvert) ; un créneau qui commence : « Créneau de {prénom} :
    /// l'ordinateur lui est prêté dans 2 min. »
    private func lendRequestArrived(_ request: LendRequest) {
        guard isLive else { return }
        if request.slot {
            show(notice: LendText.slotStarts(request.name), for: 8)
        } else if !lendPanelOpen {
            show(notice: LendText.wantsToBorrow(request.name), for: 8, opensLend: true)
        }
    }

    /// Le préavis de 2 minutes, chez l'ami : la capsule reste 10 s, le temps
    /// restant suit la fin du préavis.
    func lendNoticed(_ notice: LendNotice, text: String) {
        guard isBorrower, isLive else { return }
        lendNotice = notice
        if !text.isEmpty { show(notice: text, for: 10) }
    }

    /// La fin par le prêt, dite juste avant `.ending` : rendu, la fin est
    /// voulue ; sinon son texte reste à l'écran.
    private func lendEnded(_ end: LendEnd, text: String) {
        guard isBorrower else { return }
        if end == .left {
            // Rendu ici : fin voulue. Rendu depuis le lanceur, sur l'ordinateur :
            // « Vous avez rendu l'ordinateur. » reste à l'écran, comme les autres fins.
            if !givingBack { lendEndText = text.isEmpty ? nil : text }
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
        guard isBorrower, isLive else { return }
        giveBackAsked = false
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
        let reopen = onComeBack
        finish()
        reopen?(borrowSource)
    }

    // MARK: - Enregistrement de la session

    /// L'état a changé (jamais pour le temps qui passe) : gardé, et l'avis
    /// bref du cœur (« Enregistrement lancé sur l'ordinateur. », « {nom}
    /// enregistre la session. ») en capsule, comme ceux du partage.
    private func recordingChanged(_ state: RecordingState, notice text: String?) {
        guard !isBorrower, isLive else { return }
        recording = state
        if let text, !text.isEmpty { show(notice: text, for: 5) }
    }

    /// « Enregistrer » / « Arrêter l'enregistrement » (hôte) : la demande part
    /// à Admin, hors du fil de l'interface, par le relais quand la session y
    /// passe ; un refus se dit en avis bref (`RecordingFailure.message`).
    /// Coupé dans Admin : la raison, en avis.
    func toggleRecording() {
        guard !isVisitor, let state = recording, state.available, !recordingBusy else { return }
        if !state.allowed, !state.active {
            show(notice: state.texts.disabled, for: 5)
            return
        }
        guard let console = store.console(for: host, route: path.viaRelay ? .relay : .auto) else { return }
        recordingBusy = true
        let record = !state.active
        Task { [weak self] in
            do {
                let next = try await console.recording(deviceId: ConsoleClient.thisDeviceID, record: record)
                // L'état rendu tout de suite ; l'avis vient avec le signe de vie suivant.
                if let next, next.available { self?.recording = next }
            } catch let failure as RecordingFailure {
                if !failure.message.isEmpty { self?.show(notice: failure.message, for: 6) }
            } catch {
                self?.show(notice: error.localizedDescription, for: 6)
            }
            self?.recordingBusy = false
        }
    }

    // MARK: - PyroWave

    /// « Passer en HEVC » : le format de cette session seulement ; le choix
    /// de l'ordinateur (PyroWave) reste dans ses réglages.
    var codecOverride: VideoCodec?
    /// Le format que la Station envoie vraiment (dernier paquet de codec).
    @Published var receivedCodec: VideoCodec?
    /// Le bandeau du début de session (« PyroWave en Wi-Fi ») ou celui du
    /// réseau qui sature (« Le réseau sature ») est affiché.
    @Published var pyrowaveBanner = false
    /// Le réseau sature en PyroWave (avis du cœur) : le titre du bandeau.
    @Published var pyrowaveStrained = false
    private var pyrowaveBannerDone = false
    private var pyrowaveBannerClose: DispatchWorkItem?
    /// Profil Auto sur un autre réseau (ou par le relais) que celui du test :
    /// le bandeau discret du début (8 s ou la croix), et « Refaire le test ».
    @Published var autoNetworkBanner = false
    @Published var autoOtherNetwork = false
    private var autoNetworkChecked = false
    private var autoNetworkBannerClose: DispatchWorkItem?
    /// L'avis du repli de PyroWave est dit (une fois par session).
    private var pyrowaveFallbackTold = false

    /// Comment l'appareil joint l'ordinateur : câble, Wi-Fi (ou réseau mobile), relais.
    var pyrowaveLink: PyroWaveLink {
        #if DEBUG
        if let demoLink { return demoLink }
        #endif
        return PyroWaveLink(wired: NetworkLink.shared.wired, relay: path.viaRelay)
    }
    #if DEBUG
    var demoLink: PyroWaveLink?
    #endif

    /// La puce ambre (docs/link-ui.md, « PyroWave ») : l'image est en
    /// PyroWave sans câble ou par le relais.
    var pyrowaveChip: String? {
        receivedCodec == .pyrowave && pyrowaveLink != .cable ? PyroWaveText.chip(pyrowaveLink) : nil
    }

    /// Titre du bandeau : « PyroWave en Wi-Fi » / « … hors de chez vous »,
    /// ou « Le réseau sature ».
    var pyrowaveBannerTitle: String {
        pyrowaveStrained ? PyroWaveText.strained : PyroWaveText.chip(pyrowaveLink)
    }

    /// Une seconde de mesures : le format reçu ; une fois au début, le
    /// bandeau d'Auto sur un autre réseau que celui du test, et celui de
    /// PyroWave choisi à la main, en Wi-Fi ou par le relais.
    func followCodec() {
        guard let engine else { return }
        let codec = engine.receivedCodec
        if codec != receivedCodec { receivedCodec = codec }
        if !autoNetworkChecked {
            autoNetworkChecked = true
            if !isVisitor, store.autoOtherNetwork(host, viaRelay: path.viaRelay) { showAutoNetworkBanner() }
        }
        guard !pyrowaveBannerDone, codec == .pyrowave, host.settings.codec == .pyrowave, pyrowaveLink != .cable else { return }
        showPyroWaveBanner()
    }

    func showPyroWaveBanner(strained: Bool = false) {
        pyrowaveBannerDone = true
        pyrowaveStrained = strained
        withAnimation(.easeOut(duration: fadeDuration)) { pyrowaveBanner = true }
        // Un bandeau qui en remplace un autre garde ses 8 s.
        pyrowaveBannerClose?.cancel()
        let close = DispatchWorkItem { [weak self] in self?.closePyroWaveBanner() }
        pyrowaveBannerClose = close
        DispatchQueue.main.asyncAfter(deadline: .now() + PyroWaveSession.bannerSeconds, execute: close)
    }

    /// Le cœur dit que le réseau sature en PyroWave depuis 5 s (une fois par
    /// session, hôte seulement) : « Le réseau sature », « Passer en HEVC ».
    private func pyrowaveStrainedNotice() {
        guard !isVisitor, codecOverride == nil else { return }
        KyberLog.app.notice("PyroWave : le réseau sature, « Passer en HEVC » proposé")
        showPyroWaveBanner(strained: true)
    }

    private func showAutoNetworkBanner() {
        KyberLog.app.notice("profil Auto : autre réseau que celui du test")
        autoOtherNetwork = true
        withAnimation(.easeOut(duration: fadeDuration)) { autoNetworkBanner = true }
        autoNetworkBannerClose?.cancel()
        let close = DispatchWorkItem { [weak self] in self?.closeAutoNetworkBanner() }
        autoNetworkBannerClose = close
        DispatchQueue.main.asyncAfter(deadline: .now() + PyroWaveSession.bannerSeconds, execute: close)
    }

    /// La croix du bandeau « Autre réseau que celui du test ».
    func closeAutoNetworkBanner() {
        guard autoNetworkBanner else { return }
        withAnimation(.easeIn(duration: fadeDuration)) { autoNetworkBanner = false }
    }

    /// « Refaire le test » (bandeau, menu), ou Auto choisi sans test : le
    /// panneau du test s'ouvre à la fin de la session (le test mesure le
    /// réseau libre).
    func runAutoTest() {
        closeAutoNetworkBanner()
        openAutoTestAfterSession()
        show(notice: AutoText.afterSession, for: PyroWaveSession.bannerSeconds, icon: "sparkle")
    }

    /// La croix du bandeau.
    func closePyroWaveBanner() {
        guard pyrowaveBanner else { return }
        withAnimation(.easeIn(duration: fadeDuration)) { pyrowaveBanner = false }
    }

    /// La définition ramenée par le cœur pour tenir dans le débit (texte du
    /// cœur, langue de l'app) : dans le bandeau du début s'il est là, sinon
    /// en capsule ; nil quand elle revient à celle de l'écran.
    @Published var loweredText: String?

    private func resolutionLowered(_ mode: LoweredMode?, _ text: String?) {
        let line = text ?? mode.map { PyroWaveRules.loweredText(width: $0.width, height: $0.height, refresh: $0.refresh, mbps: $0.mbps) }
        if let mode { KyberLog.app.notice("PyroWave : définition ramenée à \(mode.width)×\(mode.height) (\(mode.mbps) Mbit/s)") }
        withAnimation(.easeOut(duration: fadeDuration)) { loweredText = line }
        if let line, !pyrowaveBanner { show(notice: line, for: PyroWaveSession.bannerSeconds, icon: "monitor") }
    }

    /// PyroWave n'a pas pu démarrer sur cet appareil : le cœur relance l'image
    /// dans le format de repli pour cette session ; l'avis du cœur, une fois.
    private func pyrowaveFallback(_ text: String) {
        guard isLive, !pyrowaveFallbackTold else { return }
        pyrowaveFallbackTold = true
        KyberLog.app.notice("PyroWave : décodeur indisponible, image dans le format de repli")
        show(notice: text, for: 6, icon: "warning")
    }

    /// « Passer en HEVC » : l'image repart en HEVC pour cette session.
    func switchToHEVC() {
        guard !isVisitor, codecOverride == nil else { return }
        codecOverride = DecoderCapabilities.current.hevc ? .hevc : .h264
        closePyroWaveBanner()
        KyberLog.app.notice("PyroWave : passage en HEVC pour cette session")
        engine?.setSettings(engineSettings)
    }

    // MARK: - Fin

    /// Les statistiques de la dernière session d'au moins 3 s (pas pour un
    /// visiteur, qui ne laisse rien sur cet appareil), et sa dernière image.
    func recordEnd(snapshotWidth: Int, hdr: Bool) {
        guard !isVisitor else { return }
        if let last = engine?.snapshot(maxWidth: snapshotWidth) { store.imagery.saveLast(last, for: host) }
        if let revealedAt {
            let m = streamMode
            host.recordSession(start: revealedAt, latency: engine?.sessionLatency, width: m.width, height: m.height, refresh: m.refresh,
                               codec: engineSettings.codec, mbps: host.settings.bitrate(width: m.width, height: m.height, refresh: m.refresh),
                               route: path.transport, hdr: hdr)
        }
        store.update(host)
    }

    // MARK: - Démonstrations

    #if DEBUG
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
        shareRoleChanged(role, previous: shareRole)
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
    #endif
}
