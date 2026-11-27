// Une session avec un ordinateur, sur l'Apple TV, vue de l'interface : le
// passage et les étapes de la connexion, le menu (bouton Retour ou L1 + R1 +
// Start + Select) et ses mesures, les avis, la fréquence du téléviseur. Le déroulé lui-même
// (préparation par Admin : écran virtuel à la taille du téléviseur, 4K
// à 60 Hz au plus, son au format de la sortie ; connexion à Kyber, choix de
// l'écran, relances, reconnexions, repli quand l'image ne passe pas, signal
// de vie, fin vérifiée) est dans le cœur Rust, par `SessionEngine`, comme
// sur Mac (docs/coeur-apple.md).
//
// L'état et les réactions communes à l'iPhone et à l'iPad (partage, prêt,
// enregistrement, PyroWave, avis, étapes) sont dans `SessionModel`
// (SessionModel.swift, audit IOS-21).

import AVFoundation
import AVKit
import Combine
import CoreMedia
import KyberKit
import SwiftUI
import UIKit

final class SessionController: SessionModel {
    /// Le passage (les deux moitiés de l'accueil) est dans la hiérarchie :
    /// jusqu'à la fin de son ouverture, puis retiré pour laisser la couche
    /// Metal seule à l'écran (voir SessionView).
    @Published private(set) var passageShown = true
    @Published private(set) var menuOpen = false
    /// Mesures de chaque seconde, dans un objet à part : seul le menu les
    /// observe ; menu fermé, la vue de session n'est plus recalculée chaque seconde.
    let meter = SessionMeter()
    private(set) var stats: VideoStats? {
        get { meter.stats }
        set { meter.stats = newValue }
    }
    private(set) var audioStats: AudioStats?
    @Published private(set) var remoteMouse = false
    /// La manette sert de souris et de clavier (actif d'office en profil Bureau).
    @Published private(set) var padMouse = false
    /// Rappel du menu, en haut de l'écran, à la fermeture du menu.
    @Published private(set) var menuHint = false
    /// HDR (docs/hdr.md) : le téléviseur accepte le HDR10, et les écrans de la
    /// session en HDR au dernier démarrage de l'image (`onHDR` du cœur).
    @Published private(set) var screenHDR = HDRDisplay.isHDR()
    @Published private(set) var hdrScreens: [Bool] = []
    private var hdrObservers: [NSObjectProtocol] = []
    /// Des critères d'affichage ont été donnés au téléviseur (à retirer à la fin).
    private var displayCriteriaSet = false

    private let onFinish: (SessionController) -> Void
    private var routeObserver: NSObjectProtocol?
    private var interruptionObserver: NSObjectProtocol?
    /// L'app est passée en arrière-plan (débit plafonné, image figée).
    private var backgroundObservers: [NSObjectProtocol] = []
    private var inBackground = false
    /// Repli quand l'image ne passe pas : un débit plus bas que sur Mac.
    private static let fallbackBitrate: UInt32 = 150_000_000

    // MARK: KyberNow Share

    /// Regarder : le choix de l'écran est ouvert, dans le menu.
    @Published var screenPanel = false
    /// Le choix de la qualité de l'image est ouvert, dans le menu.
    @Published var qualityPanel = false
    /// Le panneau de l'hôte (code, QR code, invités) est ouvert, dans le menu.
    @Published var sharePanel = false

    // MARK: Prêt de l'ordinateur

    /// Le panneau du prêt (durée, compte Steam, code, demandes) est ouvert, dans le menu.
    @Published var lendPanel = false

    // MARK: - Création

    init(host: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.onFinish = onFinish
        super.init(.computer, host: host, store: store, streamView: Self.makeView())
        remoteMouse = host.settings.profile == .bureau || host.settings.mouse == .desktop
        padMouse = host.settings.profile == .bureau
    }

    /// Une session d'invité : une Station qui a accepté un code (`Share.join`).
    /// Le pavé et la manette ne font souris que si l'hôte donne la souris :
    /// ensuite, « Pavé en souris ».
    init(guest found: ShareFound, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.onFinish = onFinish
        super.init(.guest(found), store: store, streamView: Self.makeView())
    }

    /// « Regarder » un ordinateur du compte : la session d'un invité spectateur,
    /// qui entre au départ (`start`), étapes titrées « Regarder {nom} ».
    init(watch computer: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.onFinish = onFinish
        super.init(.watch(computer), store: store, streamView: Self.makeView())
    }

    /// Une session d'emprunt : l'ordinateur d'un ami, par un code de prêt ou un
    /// compte d'ami ; l'écran virtuel que la Station fait à la taille du
    /// téléviseur. L'emprunt se fait au départ de la session (`start`).
    init(borrow source: BorrowSource, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.onFinish = onFinish
        super.init(.borrow(source), store: store, streamView: Self.makeView())
    }

    private static func makeView() -> StreamView {
        StreamView(frame: CGRect(x: 0, y: 0, width: 1920, height: 1080))
    }

    // MARK: PyroWave

    /// Images montrées et perdues des 30 dernières secondes (« Images perdues »).
    private var lossWindow: [(shown: Double, lost: Int)] = []
    private var linkWatch: NSObjectProtocol?

    /// La tuile ambre du menu, qui passe l'image en HEVC : PyroWave en Wi-Fi
    /// ou par le relais (la puce de link-ui), ou le réseau qui sature, même
    /// par câble (le bandeau ne se choisit pas à la télécommande).
    var pyrowaveTile: String? {
        guard receivedCodec == .pyrowave, codecOverride == nil, !isVisitor else { return nil }
        return pyrowaveChip ?? (pyrowaveStrained ? PyroWaveText.strained : nil)
    }

    /// « Images perdues » : part des images des 30 dernières secondes.
    var lostFramesShare: Double {
        let shown = lossWindow.reduce(0) { $0 + $1.shown }
        let lost = Double(lossWindow.reduce(0) { $0 + $1.lost })
        return shown + lost > 0 ? lost / (shown + lost) * 100 : 0
    }

    /// Une seconde de mesures : les pertes, le format reçu et les bandeaux du début.
    private func videoStats(_ stats: VideoStats) {
        self.stats = stats
        lossWindow.append((stats.fps, stats.lostFrames))
        if lossWindow.count > 30 { lossWindow.removeFirst(lossWindow.count - 30) }
        followCodec()
    }

    /// Le débit que l'Apple TV tient vers l'ordinateur : en Ethernet, le
    /// Gigabit de l'Apple TV (≈ 940 Mbit/s utiles) ; en Wi-Fi, inconnu (tvOS
    /// ne donne pas le débit du lien) : la Station et le débit réglé plafonnent seuls.
    static var linkMbps: UInt32? { NetworkLink.shared.wired ? 940 : nil }

    // MARK: HDR

    /// Ce que la Station dit du HDR. Un ami qui emprunte n'a pas l'info de la
    /// Station : la puce reste active et le cœur décide au départ de l'image.
    private var hdrOffer: HDROffer? {
        isBorrower ? HDROffer(available: true) : store.status[host.id]?.info?.hdr
    }

    /// « HDR » du menu de session : caché pour un invité (c'est automatique pour lui)
    /// et sur un téléviseur sans HDR ; l'ami qui emprunte l'a, comme un hôte.
    var hdrState: HDRState {
        guard !isGuest else { return .hidden }
        return HDRSupport.state(offer: hdrOffer, profile: engineSettings.profile, codec: engineSettings.codec, screenHDR: screenHDR)
    }

    /// Le HDR est demandé pour cette session.
    var hdrOn: Bool { hdrState.on(host.settings.hdr) }

    /// Un écran de la session est en HDR : « HDR » dans les statistiques.
    var streamHDR: Bool { hdrScreens.contains(true) }

    /// « HDR » coché ou décoché dans le menu : gardé pour l'ordinateur (pas
    /// pour un ami qui emprunte), le cœur relance l'image, le téléviseur suit.
    func setHDR(_ on: Bool) {
        guard !isGuest, hdrState.available, on != host.settings.hdr else { return }
        host.settings.hdr = on
        if !isBorrower { store.update(host) }
        engine?.setHDR(on)
        matchDisplay(streamMode.refresh)
    }

    /// Le téléviseur gagne ou perd le HDR (sortie changée, téléviseur éteint) :
    /// le cœur fait repasser l'image en SDR ou en HDR, sans message ; le choix reste.
    private func screenHDRChanged() {
        let hdr = HDRDisplay.isHDR()
        guard hdr != screenHDR else { return }
        screenHDR = hdr
        engine?.screenHDRChanged(0, hdr: hdr, luminance: HDRCalibrationStore.session(.current(), hdr: hdr))
        if isLive, streamMode.refresh > 0 { matchDisplay(streamMode.refresh) }
    }

    // MARK: - Déroulé

    func start() {
        let settings = host.settings
        let mode = TVDisplay.mode(for: settings)
        streamMode = mode
        modeLabel = SessionModel.modeLabel(mode.width, mode.height, mode.refresh)
        matchDisplay(mode.refresh)
        hdrObservers = HDRDisplay.observe { [weak self] in
            MainActor.assumeIsolated { self?.screenHDRChanged() }
        }
        let gamepads = GamepadBridge.shared
        gamepads.onMenuRequest = { [weak self] in self?.menuRequested() }
        gamepads.menuFromRemote = prefs.menuBack
        gamepads.menuFromCombo = prefs.menuCombo
        gamepads.rumbleEnabled = prefs.rumble
        gamepads.padMouseSpeed = prefs.padMouseSpeed.factor
        // Manette en souris face à une Station Mac : ⌘ ⇥ et ⌘ Espace (invité, ami : d'un PC).
        gamepads.macKeys = !isVisitor && host.isMac
        padsWatch = gamepads.$pads.receive(on: RunLoop.main).sink { [weak self] pads in
            if !pads.isEmpty { self?.padsUnavailableNotice() }
        }
        streamView.padMouse = padMouse
        streamView.remoteMouse = remoteMouse
        streamView.appleISOKeyboard = prefs.appleISOKeyboard
        routeObserver = NotificationCenter.default.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] _ in
            Task { @MainActor in self?.route = AudioRoute.current }
        }
        watchCameras()
        // Siri, une autre app qui prend le son : à la fin de l'interruption,
        // la session audio repart.
        interruptionObserver = NotificationCenter.default.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { note in
            let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt
            guard raw.flatMap(AVAudioSession.InterruptionType.init(rawValue:)) == .ended else { return }
            KyberLog.app.notice("fin de l'interruption du son : session audio reprise")
            try? AVAudioSession.sharedInstance().setActive(true)
        }
        observeBackground()
        let tv = TVDisplay.native
        let hdr = HDRDisplay.isHDR() && host.settings.hdr
        begin(shareScreen: ShareScreen(pixelWidth: tv.width, pixelHeight: tv.height, refresh: tv.refresh),
              lendScreen: LendScreen(width: mode.width, height: mode.height, refresh: mode.refresh, hdr: hdr,
                                     luminance: HDRCalibrationStore.session(.current(), hdr: hdr))) { [weak self] password in
            self?.launch(settings: settings, password: password, mode: mode)
        }
    }

    /// L'app passe en arrière-plan (bouton TV, autre app) : débit plafonné à
    /// celui de l'Apple TV (8 Mbit/s, le cœur) et image figée sans panne,
    /// rendus au retour (TRANS-40) ; rien ne reste enfoncé sur l'ordinateur.
    private func observeBackground() {
        let center = NotificationCenter.default
        backgroundObservers = [
            center.addObserver(forName: UIScene.willDeactivateNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self, !self.inBackground, SessionRules.reactsToBackground(self.phase.stage) else { return }
                    KyberLog.app.notice("app en arrière-plan : image figée, débit réduit")
                    self.inBackground = true
                    // Les manettes quittent le jeu : tout est relâché sur l'ordinateur.
                    GamepadBridge.shared.setCaptured(true)
                    self.engine?.setBackground(true)
                }
            },
            center.addObserver(forName: UIScene.didActivateNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self, self.inBackground else { return }
                    KyberLog.app.notice("app au premier plan : image et débit repris")
                    self.inBackground = false
                    if !self.menuOpen { GamepadBridge.shared.setCaptured(false) }
                    self.engine?.setBackground(false)
                }
            },
        ]
    }

    /// La session du cœur : celle de l'ordinateur, d'un invité ou d'un ami qui emprunte.
    private func launch(settings: ConnectionSettings, password: String, mode: (width: Int, height: Int, refresh: Int)) {
        let engine = SessionEngine(view: streamView)
        wire(engine)
        engine.onMode = { [weak self] mode, _ in
            guard let self else { return }
            self.streamMode = (mode.width, mode.height, mode.refresh)
            self.modeLabel = SessionModel.modeLabel(mode.width, mode.height, mode.refresh)
        }
        engine.onVideoStats = { [weak self] in self?.videoStats($0) }
        engine.onHDR = { [weak self] screens in self?.hdrScreens = screens }
        engine.onAudioStats = { [weak self] in self?.audioStats = $0 }

        // Son : celui de la sortie (ampli 5.1 ou 7.1, sinon stéréo), ou resté sur l'ordinateur.
        let channels = settings.audio == .off ? 0 : settings.audio.channels
        var params = self.params(mode: mode, password: password, channels: channels)
        params.clipboard = false
        params.vsync = true
        params.fallbackBitrate = Self.fallbackBitrate
        params.screenHDR = screenHDR
        // Les lumières du téléviseur (réglées à l'œil, sinon par défaut), gardées jusqu'à la fin.
        params.screenLuminance = HDRCalibrationStore.session(.current(), hdr: screenHDR)
        params.hdrOffer = hdrOffer
        KyberLog.app.notice("session : \(mode.width)×\(mode.height) à \(mode.refresh) Hz, son \(channels) canaux, \(settings.consoleProfile, privacy: .public)")
        do {
            try engine.start(params)
            // PyroWave (« Définition ramenée ») : le débit du lien, juste après le démarrage.
            engine.setLinkMbps(Self.linkMbps)
            linkWatch = NotificationCenter.default.addObserver(forName: NetworkLink.changed, object: nil, queue: .main) { [weak self] _ in
                Task { @MainActor in self?.engine?.setLinkMbps(Self.linkMbps) }
            }
        } catch {
            fail(error.localizedDescription)
        }
    }

    /// Le téléviseur passe à la fréquence de la session (réglage « Ajuster la
    /// fréquence » de tvOS) et en HDR10 quand le HDR est demandé (réglage
    /// « Adapter la plage dynamique » ; sans lui, le HDR suit la sortie
    /// choisie dans tvOS), puis revient à la fin (`hz` nil).
    private func matchDisplay(_ hz: Int?) {
        guard let window = UIApplication.shared.connectedScenes.compactMap({ ($0 as? UIWindowScene)?.keyWindow }).first else { return }
        let hdr = hdrOn
        guard let hz, prefs.matchFrameRate || hdr else {
            if displayCriteriaSet {
                window.avDisplayManager.preferredDisplayCriteria = nil
                displayCriteriaSet = false
            }
            return
        }
        // « Caler le téléviseur sur la session » coupé, HDR demandé : la
        // fréquence du téléviseur telle qu'elle est, pour ne demander que le
        // HDR (un critère porte toujours une fréquence) ; l'utilisateur a
        // choisi de ne pas la faire suivre la session.
        let rate = prefs.matchFrameRate ? hz : TVDisplay.native.refresh
        // Image à la définition de la session : le téléviseur garde sa
        // définition ; la fréquence suit, et la plage dynamique pour le HDR.
        guard let criteria = HDRDisplay.criteria(refresh: rate, width: streamMode.width, height: streamMode.height, hdr: hdr) else { return }
        window.avDisplayManager.preferredDisplayCriteria = criteria
        displayCriteriaSet = true
        KyberLog.app.notice("téléviseur : \(rate) Hz\(hdr ? ", HDR10" : "", privacy: .public) demandés ; \(HDRDisplay.describeTV(window), privacy: .public)")
    }

    /// La première image : aucune capsule à l'arrivée (les mesures sont dans
    /// le menu), le passage quitte la hiérarchie une fois ouvert.
    override func revealed() { dismissPassage() }
    override var revealDuration: Double { 1.5 }
    override var fadeDuration: Double { 0.35 }
    /// Le téléviseur ne change pas en session : pas d'adaptation à dire.
    override var tellsAdaptation: Bool { false }
    override var sharePanelOpen: Bool { sharePanel }
    override var lendPanelOpen: Bool { lendPanel }
    override func restoreOfferHint(_ offer: String) -> String { String(localized: " · « \(offer) » dans le menu") }
    override func openAutoTestAfterSession() {
        AppModel.current?.autoTest.openAfterSession(host)
        closeMenu()
    }

    /// Le passage a fini de s'ouvrir (1,5 s) : il quitte la hiérarchie.
    private func dismissPassage() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.7) { [weak self] in
            guard let self, self.revealedAt != nil else { return }
            self.passageShown = false
        }
    }

    /// Les droits de l'invité ont changé (`SessionModel`) : le clavier et la
    /// souris retirés, le pavé et la manette ne font plus souris.
    override func rightsChanged(_ role: ShareRole) {
        guard !role.keyboardAndMouse else { return }
        if remoteMouse { toggleRemoteMouse() }
        if padMouse { togglePadMouse() }
    }

    /// Un iPhone ou un iPad est relié comme caméra (Caméra de continuité) :
    /// tenu à jour quand il se relie ou s'en va, pas relu à chaque rendu.
    @Published private(set) var cameraReady = !CameraCapture.cameras().isEmpty
    private var cameraWatch: [NSObjectProtocol] = []

    private func watchCameras() {
        let center = NotificationCenter.default
        cameraWatch = [AVCaptureDevice.wasConnectedNotification, AVCaptureDevice.wasDisconnectedNotification].map { name in
            center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.cameraReady = !CameraCapture.cameras().isEmpty }
            }
        }
    }

    /// La tuile « Caméra » : allume ou éteint la caméra de l'iPhone choisi.
    /// `chosen` : celle que le choix de tvOS vient de rendre ; sinon celle que
    /// le système préfère, puis la première.
    func toggleCamera(chosen: String? = nil) {
        guard let engine, cameraOffered else { return }
        if cameraState == .off {
            let list = CameraCapture.cameras()
            cameraReady = !list.isEmpty
            let preferred = AVCaptureDevice.systemPreferredCamera?.uniqueID
            let id = [chosen, preferred].compactMap { $0 }.first { id in list.contains { $0.id == id } } ?? chosen ?? list.first?.id
            engine.cameraOn(id: id, resolution1080: host.settings.camera1080)
        } else {
            engine.cameraOff()
        }
    }

    // MARK: - Menu

    /// Bouton Retour de la télécommande, ou combinaison à la manette.
    private func menuRequested() {
        // Pendant la reconnexion aussi : le menu a « Se déconnecter ».
        guard SessionRules.opensMenu(phase.stage) else { return }
        if menuOpen {
            closeMenu()
        } else {
            openMenu()
        }
    }

    func openMenu() {
        guard !menuOpen else { return }
        GamepadBridge.shared.setCaptured(true)
        withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.5)) { menuOpen = true }
        route = AudioRoute.current
    }

    func closeMenu() {
        guard menuOpen else { return }
        withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 0.5)) { menuOpen = false }
        GamepadBridge.shared.setCaptured(false)
        withAnimation(.easeOut(duration: 0.35)) { menuHint = true }
        // Le rappel d'avant ne cache pas celui-ci trop tôt.
        menuHintClose?.cancel()
        let close = DispatchWorkItem { [weak self] in
            withAnimation(.easeIn(duration: 0.35)) { self?.menuHint = false }
        }
        menuHintClose = close
        DispatchQueue.main.asyncAfter(deadline: .now() + 4, execute: close)
    }

    private var menuHintClose: DispatchWorkItem?

    /// Retour de la télécommande quand l'app a la main (`SessionView`).
    func exitCommand() {
        if menuOpen {
            closeMenu()
        } else if case .failed = phase {
            finish()
        } else if SessionRules.opensMenu(phase.stage) {
            // Reconnexion : le menu, d'où « Se déconnecter » sort.
            openMenu()
        } else {
            // Préparation, connexion : l'attente est annulée.
            end()
        }
    }

    func togglePadMouse() {
        padMouse.toggle()
        streamView.padMouse = padMouse
    }

    func toggleRemoteMouse() {
        remoteMouse.toggle()
        streamView.remoteMouse = remoteMouse
    }

    func type(_ text: String) {
        streamView.type(text)
    }

    var sessionMinutes: Int { Int(Date().timeIntervalSince(startedAt) / 60) }

    #if DEBUG
    enum Demo { case connecting, restarting, passage, menu }

    /// Démonstration (simulateur), sans ordinateur : les étapes de la
    /// connexion (arrêtées sur « Connexion »), l'image seule, ou le menu.
    func demo(_ kind: Demo) {
        phase = .preparing
        // KyberNow Share : le partage et le prêt se jouent ici, sans Station.
        share.demo = true
        lend.demo = true
        streamView.startTestPattern()
        if kind == .connecting || kind == .restarting {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                self?.phase = .connecting
                if kind == .restarting { self?.stationRestarting = true }
            }
            return
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6) { [weak self] in
            guard let self else { return }
            withAnimation(.timingCurve(0.2, 0.9, 0.1, 1, duration: 1.5)) {
                self.revealedAt = Date()
                self.phase = .streaming
            }
            self.dismissPassage()
            self.startedAt = Date().addingTimeInterval(-42 * 60)
            self.modeLabel = String(localized: "\(3840) × \(2160) à \(60) Hz")
            var stats = VideoStats()
            stats.latencyMs = 9.1
            stats.fps = 60
            stats.mbps = 78
            stats.target = TargetBitrate(bitrate: 60_000_000, profile: 60_000_000, reason: 0)
            stats.format = self.hdrOn ? "4:2:0 10 bits" : "4:2:0 8 bits"
            stats.decodeMs = 2.1
            stats.displayMs = 7.4
            // Trajet d'une image ; `-demoChain NO` : une Station qui n'envoie pas ses étapes.
            if UserDefaults.standard.object(forKey: "demoChain") == nil || UserDefaults.standard.bool(forKey: "demoChain") {
                var c = ChainTimes()
                (c.capture, c.encode, c.send, c.network, c.decode, c.display) = (0.9, 3.4, 0.6, 1.3, 2.1, 5.3)
                stats.chain = c
            }
            // `-demoPyroWave wifi|relay` : l'image en PyroWave choisi à la main, en Wi-Fi ou par le relais
            // (bandeau du début, tuile du menu, « Images perdues »).
            if let pw = UserDefaults.standard.string(forKey: "demoPyroWave") {
                self.demoLink = pw == "relay" ? .relay : .wifi
                if pw == "relay" { self.path = RouteInfo(transport: .relay, relay: Relay.defaultServer, rttClient: 24, rttStation: 11) }
                self.host.settings.codec = .pyrowave
                self.receivedCodec = .pyrowave
                stats.mbps = 290
                stats.target = TargetBitrate(bitrate: 320_000_000, profile: 320_000_000, reason: 3)
                stats.decodeMs = 0.9
                stats.lostFrames = 1
                self.lossWindow = Array(repeating: (60, 0), count: 24) + [(59, 1), (60, 0), (58, 2), (60, 0), (59, 1), (60, 0)]
                // `-demoStrained YES` : le bandeau du réseau qui sature à la place de celui du début.
                self.pyrowaveStrained = UserDefaults.standard.bool(forKey: "demoStrained")
                withAnimation(.easeOut(duration: 0.35)) { self.pyrowaveBanner = kind != .menu }
                // `-demoLowered YES` : la définition ramenée (bandeau, ligne des statistiques).
                if UserDefaults.standard.bool(forKey: "demoLowered") {
                    stats.lowered = LoweredMode(width: 2560, height: 1440, refresh: 60, mbps: 300)
                    self.loweredText = PyroWaveRules.loweredText(width: 2560, height: 1440, refresh: 60, mbps: 300)
                }
            }
            self.stats = stats
            // Le HDR demandé (`-hdrScreen YES`) : l'écran de la démonstration est « en HDR ».
            self.hdrScreens = [self.hdrOn]
            if kind == .menu {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.openMenu() }
            }
            // `-demoQuality YES` (invité, avec `-screen guest-menu`) : le choix de la qualité de l'image ouvert.
            if UserDefaults.standard.bool(forKey: "demoQuality") {
                DispatchQueue.main.asyncAfter(deadline: .now() + 1) { self.qualityPanel = true }
            }
            // `-demoRelay YES` : la session passe par le relais (puce « Par le relais », « Chemin » des mesures).
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
                    self.openMenu()
                    self.sharePanel = true
                }
            }
            // `-demoLend code|request` : le panneau du prêt ouvert, avec un code créé, ou une demande.
            if let kind = UserDefaults.standard.string(forKey: "demoLend"), !self.isVisitor {
                Task {
                    if kind == "code" { await self.lend.createCode() }
                    if kind == "request" { self.demoLendRequest(name: "Léa") }
                    self.openMenu()
                    self.lendPanel = true
                }
            }
            // `-demoShare YES` : le partage ouvert, quatre invités, le panneau de l'hôte.
            if UserDefaults.standard.bool(forKey: "demoShare") {
                Task {
                    await self.share.openIfNeeded()
                    // Léa et Sam reçoivent une image à part (leur qualité se voit) ; Hugo suit l'hôte ; Zoé : rien de su.
                    self.share.demoAddGuest(name: "Léa", app: "ios", role: .player, image: ShareImage.demo("1080p60", 1920, 1080, 60, follows: false))
                    self.share.demoAddGuest(name: "Hugo", app: "tvos", image: ShareImage.demo("host", 3840, 2160, 60, follows: true))
                    self.share.demoAddGuest(name: "Sam", app: "web", role: .playerKbm, image: ShareImage.demo("720p30", 1280, 720, 30, follows: false))
                    self.share.demoAddGuest(name: "Zoé", app: "mac", role: .kbm, image: ShareImage.demo("auto"))
                    self.openMenu()
                    self.sharePanel = true
                }
            }
            // `-demoRoleThen spectator|player|kbm|player_kbm|end-stopped|end-removed` : ce que l'hôte fait 6 s plus tard (invité).
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
    /// Démonstration : regarder un ordinateur du compte, deux écrans dans « Écran ».
    func demoWatch(menu: Bool) {
        let screens = [ShareWatchScreen(demoID: 1, label: "Écran 1 — 3840 × 2160"), ShareWatchScreen(demoID: 2, label: "Écran 2 — 2560 × 1440")]
        guest = ShareFound(demoComputer: host.name, host: host.name, role: .spectator, watch: ShareWatch(demoScreens: screens))
        demoGuest(role: .spectator, menu: menu)
    }

    func demoGuest(role: ShareRole, menu: Bool) {
        shareRole = role
        // La qualité reçue dans le menu : une image à part en 1080p · 60 Hz.
        shareImage = ShareImage.demo("1080p60", 1920, 1080, 60, follows: false)
        demo(menu ? .menu : .passage)
    }

    /// Démonstration : la session d'emprunt (sans Station) : `waiting`
    /// (« En attente de {hôte}… »), sinon en image (`menu` : le menu ouvert),
    /// le prêt jusqu'à `until` ; `then` : `notice`, `taken_back`, `lost` 6 s plus tard.
    func demoBorrow(waiting: Bool, menu: Bool, until: Date?, then: String?) {
        let host = borrowSource?.hostHint.nonEmpty ?? "Matthieu"
        if waiting {
            phase = .preparing
            lend.demo = true
            streamView.startTestPattern()
            borrowFlow?.demo(nil, waiting: host)
            return
        }
        loan = LendLoan(demoComputer: "PC-SALON", host: host, until: until)
        demo(menu ? .menu : .passage)
        // `-demoGiveBack YES` : « Rendre l'ordinateur à {hôte} ? » ouvert dans le menu.
        if menu, UserDefaults.standard.bool(forKey: "demoGiveBack") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) { [weak self] in self?.giveBackAsked = true }
        }
        guard let then else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 6) { [weak self] in self?.demoLend(then) }
    }

    #endif

    // MARK: - Fin

    /// Fin de session : le cœur rend la main à Admin (écrans et son de
    /// l'ordinateur rétablis, latence de la session donnée à ses appareils).
    /// Une session d'au moins 3 s devient la « Dernière session » des Statistiques.
    override func end() {
        guard phase != .ending && phase != .ended else { return }
        // « Annuler » pendant la préparation ou l'attente de l'hôte : la demande est retirée.
        if loan == nil { borrowFlow?.cancel() }
        GamepadBridge.shared.onMenuRequest = nil
        padsWatch = nil
        GamepadBridge.shared.macKeys = false
        streamView.padMouse = false
        // Dernière image et statistiques (rien pour un visiteur).
        recordEnd(snapshotWidth: 1920, hdr: streamHDR)
        withAnimation(.easeInOut(duration: 0.4)) {
            phase = .ending
            menuOpen = false
        }
        tearDown()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { [weak self] in self?.finish() }
    }

    override func finish() {
        if let linkWatch { NotificationCenter.default.removeObserver(linkWatch) }
        linkWatch = nil
        guard phase != .ended else { return }
        phase = .ended
        if !isVisitor { store.setSessionViaRelay(host, false) }
        // L'emprunt pas encore abouti : la demande est retirée chez l'hôte.
        borrowFlow?.cancel()
        onFinish(self)
    }

    override func fail(_ message: String) {
        KyberLog.app.error("session : \(message, privacy: .public)")
        GamepadBridge.shared.onMenuRequest = nil
        padsWatch = nil
        GamepadBridge.shared.macKeys = false
        tearDown()
        withAnimation(Theme.easeOut) {
            menuOpen = false
            phase = .failed(message)
        }
    }

    /// Ce que `end()` et `fail()` défont tous les deux (avant, l'échec gardait
    /// l'observateur du son et la manette en souris).
    private func tearDown() {
        streamView.padMouse = false
        streamView.detach()
        engine?.stop()
        matchDisplay(nil)
        hdrObservers.forEach(NotificationCenter.default.removeObserver)
        hdrObservers = []
        if let routeObserver { NotificationCenter.default.removeObserver(routeObserver) }
        routeObserver = nil
        if let interruptionObserver { NotificationCenter.default.removeObserver(interruptionObserver) }
        interruptionObserver = nil
        cameraWatch.forEach(NotificationCenter.default.removeObserver)
        cameraWatch = []
        backgroundObservers.forEach(NotificationCenter.default.removeObserver)
        backgroundObservers = []
        if inBackground {
            inBackground = false
            GamepadBridge.shared.setCaptured(false)
        }
    }
}

/// Les mesures de la session (image), observées par le menu seulement.
@MainActor
final class SessionMeter: ObservableObject {
    @Published var stats: VideoStats?
}
