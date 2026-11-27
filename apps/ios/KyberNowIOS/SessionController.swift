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
//
// L'état et les réactions communes à l'Apple TV (partage, prêt,
// enregistrement, PyroWave, avis, étapes) sont dans `SessionModel`
// (apps/tvos/KyberNowTV/SessionModel.swift, audit IOS-21).

import AVFoundation
import Combine
import GameController
import KyberKit
import SwiftUI
import UIKit

final class SessionController: SessionModel {
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

    /// Les caméras de cet appareil (avant, arrière), relues à l'ouverture du menu.
    @Published private(set) var cameras: [CameraChoice] = []
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
    /// HDR (docs/hdr.md) : l'écran qui montre l'image est HDR ; un écran au
    /// moins de la session est en HDR (`onHDR` du cœur, statistiques).
    @Published private(set) var screenHDR = false
    /// Les valeurs HDR de cet écran, dites au cœur (automatiques ou réglées à l'œil).
    private var screenLuminance: HDRLuminance?
    @Published private(set) var sessionHDR = false

    private let onFinish: (SessionController) -> Void
    private var observers: [NSObjectProtocol] = []
    private var externalWatch: AnyCancellable?
    /// La fenêtre de la session est passée en arrière-plan (débit plafonné, image figée).
    private var inBackground = false

    // MARK: KyberNow Share

    /// Le panneau de l'hôte (code, QR code, invités) est ouvert.
    @Published var shareSheet = false

    // MARK: Prêt de l'ordinateur

    /// Face à une Station Mac : menu Touches et rangée du clavier du Mac
    /// (docs/link-ui.md). Un invité ou un ami ne connaît pas le système de
    /// l'ordinateur : touches d'un PC.
    var macKeys: Bool { !isVisitor && host.isMac }
    /// Le panneau « Prêter l'ordinateur… » est ouvert.
    @Published var lendSheet = false

    // MARK: - Création

    init(host: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.onFinish = onFinish
        super.init(.computer, host: host, store: store, streamView: Self.makeView())
    }

    /// Une session d'invité : une Station qui a accepté un code (`Share.join`).
    /// Pas d'écran virtuel, la souris du bureau, rien de partagé.
    init(guest found: ShareFound, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.onFinish = onFinish
        super.init(.guest(found), store: store, streamView: Self.makeView())
        virtualDisplay = false
    }

    /// « Regarder » un ordinateur du compte : la session d'un invité spectateur,
    /// qui entre au départ (`start`), étapes titrées « Regarder {nom} ».
    init(watch computer: Host, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.onFinish = onFinish
        super.init(.watch(computer), store: store, streamView: Self.makeView())
        virtualDisplay = false
    }

    /// Une session d'emprunt : l'ordinateur d'un ami, par un code de prêt ou un
    /// compte d'ami. L'emprunt se fait au départ de la session (`start`) ;
    /// l'écran virtuel que la Station fait à la taille de cet appareil.
    init(borrow source: BorrowSource, store: HostStore, onFinish: @escaping (SessionController) -> Void) {
        self.onFinish = onFinish
        super.init(.borrow(source), store: store, streamView: Self.makeView())
    }

    private static func makeView() -> StreamView {
        StreamView(frame: CGRect(x: 0, y: 0, width: 874, height: 402))
    }

    // MARK: - HDR

    /// L'écran qui montre l'image : l'écran externe s'il la porte, sinon celui de l'appareil.
    private var imageScreen: UIScreen {
        external ? ExternalDisplay.shared.screen ?? deviceScreen : deviceScreen
    }

    /// « HDR » de la barre : comme la puce de l'accueil. Un invité du partage
    /// n'en a pas (c'est automatique) ; l'ami qui emprunte l'a, comme un hôte :
    /// sans info de la Station du prêt ici, la règle complète est celle du cœur.
    var hdrState: HDRState {
        if isGuest { return .hidden }
        if isBorrower {
            return HDRSupport.state(offer: HDROffer(available: true), profile: engineSettings.profile, codec: engineSettings.codec, screenHDR: screenHDR)
        }
        return host.settings.hdrState(store.status[host.id]?.info, screenHDR: screenHDR)
    }

    /// « HDR » coché dans la barre.
    var hdrOn: Bool { hdrState.on(host.settings.hdr) }

    /// « HDR » de la barre : gardé pour l'ordinateur (pas le profil), puis le
    /// cœur relance l'image (étape « Premières images »).
    /// La caméra choisie dans la liste, gardée pour cet appareil (pas par ordinateur).
    static var cameraChoice: String? {
        get { UserDefaults.standard.string(forKey: "kybernow.camera") }
        set { UserDefaults.standard.set(newValue, forKey: "kybernow.camera") }
    }

    /// Le bouton « Caméra » : allume ou éteint la caméra.
    func toggleCamera() {
        guard let engine, cameraOffered else {
            KyberLog.app.notice("caméra : bouton sans offre de l'ordinateur")
            return
        }
        if cameraState == .off {
            cameras = CameraCapture.cameras()
            let id = Self.cameraChoice.flatMap { id in cameras.contains { $0.id == id } ? id : nil }
            engine.cameraOn(id: id, resolution1080: host.settings.camera1080)
        } else {
            engine.cameraOff()
        }
    }

    /// « Caméra avant » / « Caméra arrière » : gardée, prise tout de suite si elle est allumée.
    func pickCamera(_ id: String) {
        Self.cameraChoice = id
        guard let engine, cameraState != .off else { return }
        engine.cameraOn(id: id, resolution1080: host.settings.camera1080)
    }

    var currentCamera: String? { engine?.cameraID ?? Self.cameraChoice ?? CameraCapture.defaultCamera() }

    func setHDR(_ on: Bool) {
        guard !isGuest, hdrState.available, on != host.settings.hdr else { return }
        host.settings.hdr = on
        if !isBorrower { store.update(host) }
        engine?.setHDR(on)
    }

    // MARK: - Micro (docs/micro.md)

    /// Le micro tel que le cœur le montre (`MicUI` : « Micro » de la barre,
    /// tuile « Envoyé », « Envoi… », « Coupé », icône près de l'onglet).
    private lazy var micUI = MicUI(on: host.settings.microphone)
    @Published private(set) var mic = MicView()

    /// « Micro » allumé : le choix de l'ordinateur, sauf refus.
    var micOn: Bool { mic.lit }
    /// Envoyé à l'ordinateur : l'icône de micro près de l'onglet de la session.
    var micSending: Bool { mic.sent }
    /// Un invité que l'hôte ne laisse pas parler n'a pas « Micro ».
    var micShown: Bool { mic.shown }
    /// Le sous-titre de la tuile : « Envoyé », « Envoi… » ou « Coupé ».
    var micSubtitle: String { mic.tile }

    private func refreshMic() { mic = micUI.view(shortcut: "", guest: isGuest) }

    /// « Micro » touché : allumé ou coupé, gardé pour l'ordinateur ;
    /// l'autorisation du système est demandée la première fois qu'on
    /// l'allume. Refusé par l'ordinateur : sa raison, en avis.
    func toggleMicrophone() {
        switch micUI.tap() {
        case .refused: if let reason = mic.reason { show(notice: reason, for: 5) }
        case .turnOn: setMicrophone(true)
        case .turnOff: setMicrophone(false)
        }
    }

    func setMicrophone(_ on: Bool) {
        guard on != host.settings.microphone else { return }
        guard on else { return applyMicrophone(false) }
        MicrophoneAccess.request { [weak self] granted in
            guard let self else { return }
            guard granted else {
                self.micUI.denied()
                self.refreshMic()
                return self.show(notice: MicText.permission, for: 6, opensSettings: true)
            }
            self.applyMicrophone(true)
        }
    }

    private func applyMicrophone(_ on: Bool) {
        host.settings.microphone = on
        if !isVisitor { store.update(host) }
        micUI.set(on)
        refreshMic()
        engine?.setMicrophone(on)
    }

    /// Le cœur dit l'état du micro (la capture suit déjà dans `SessionEngine`) ;
    /// l'avis « Micro envoyé à {nom}. » vient quand il le prend.
    private func microphoneChanged(_ state: MicState) {
        let notice = micUI.changed(state, computer: host.name)
        refreshMic()
        if let notice { show(notice: notice, for: 3) }
    }

    /// Réglages de l'app changés (micro choisi, écho et bruit) : la capture reprend avec eux.
    func microphoneSettingsChanged() { engine?.microphoneSettingsChanged() }

    // MARK: - Stylet : Pression / Souris (docs/link-ui.md, « Stylet »)

    /// La tuile « Stylet », textes du cœur ; relue à l'ouverture de la barre
    /// et quand l'Apple Pencil touche ou survole l'image.
    @Published private(set) var pen = PenView()

    /// La tuile a sa place : l'Apple Pencil a servi (dans cette session, ou
    /// déjà dans l'app sur cet iPad) et l'ordinateur reçoit le stylet ;
    /// seulement avec le clavier et la souris.
    var penShown: Bool { pen.shown && (!isGuest || shareRole.keyboardAndMouse) }

    func refreshPen() {
        let view = engine?.penView(deviceHasPen: PenView.deviceHasPen) ?? PenView()
        if view != pen { pen = view }
    }

    /// L'Apple Pencil touche ou survole l'image : retenu pour cet iPad.
    private func pencilArrived() {
        if !PenView.deviceHasPen { PenView.deviceHasPen = true }
        refreshPen()
    }

    /// « Pression » ou « Souris » (tuile, réglage de l'ordinateur) : gardé
    /// pour l'ordinateur, appliqué tout de suite.
    func setPenMode(_ mode: PenMode) {
        guard mode != host.settings.pen else { return }
        host.settings.pen = mode
        if !isVisitor { store.update(host) }
        engine?.setPenMode(mode)
        refreshPen()
    }

    /// La tuile « Stylet » touchée : bascule entre Pression et Souris.
    func togglePenMode() { setPenMode(host.settings.pen.toggled) }

    /// « HDR » inactif touché dans la barre : sa raison, en avis.
    func explainHDR() {
        if let reason = hdrState.reason { show(notice: reason, for: 4) }
    }

    /// L'écran de l'image gagne ou perd le HDR (écran externe, réglages) :
    /// le cœur fait repasser l'image en SDR ou en HDR, sans message.
    private func updateScreenHDR() {
        let hdr = HDRDisplay.isHDR(imageScreen)
        let luminance = HDRCalibrationStore.session(.current(imageScreen), hdr: hdr)
        guard hdr != screenHDR || luminance != screenLuminance else { return }
        screenHDR = hdr
        screenLuminance = luminance
        engine?.screenHDRChanged(0, hdr: hdr, luminance: luminance)
    }

    // MARK: - Déroulé

    func start() {
        let settings = host.settings
        // « Comme cet écran » : l'écran externe s'il est branché, sinon celui de l'appareil.
        let mode = ExternalDisplay.shared.mode(for: settings) ?? DeviceDisplay.mode(for: settings, screen: deviceScreen)
        modeLabel = SessionModel.modeLabel(mode.width, mode.height, mode.refresh)
        streamMode = mode
        configureView()
        observeApp()
        if ExternalDisplay.shared.scene != nil { useExternal(true, adapt: false) }
        // Après la mise à jour : `@Published` émet avant l'affectation, et
        // `useExternal` relit l'écran (mode et HDR de la TV branchée).
        externalWatch = ExternalDisplay.shared.$scene.dropFirst().receive(on: DispatchQueue.main).sink { [weak self] scene in
            self?.useExternal(scene != nil, adapt: true)
        }
        padsWatch = GamepadBridge.shared.$pads.receive(on: RunLoop.main).sink { [weak self] pads in
            if !pads.isEmpty { self?.padsUnavailableNotice() }
        }
        let hdr = HDRDisplay.isHDR(imageScreen) && host.settings.hdr
        begin(shareScreen: DeviceDisplay.shareScreen(),
              lendScreen: LendScreen(width: mode.width, height: mode.height, refresh: mode.refresh, hdr: hdr,
                                     luminance: HDRCalibrationStore.session(.current(imageScreen), hdr: hdr))) { [weak self] password in
            self?.startEngine(mode: mode, password: password)
        }
    }

    /// La session du cœur : celle de l'ordinateur, d'un invité ou d'un ami qui emprunte.
    private func startEngine(mode: (width: Int, height: Int, refresh: Int), password: String) {
        let settings = host.settings
        let engine = SessionEngine(view: streamView)
        wire(engine)
        engine.onDisplays = { [weak self] in self?.screens = $0 }
        engine.onShownDisplay = { [weak self] in self?.shownDisplay = $0.id }
        engine.onMode = { [weak self] mode, virtualDisplay in
            guard let self else { return }
            self.modeLabel = SessionModel.modeLabel(mode.width, mode.height, mode.refresh)
            self.streamMode = (mode.width, mode.height, mode.refresh)
            self.virtualDisplay = virtualDisplay
        }
        engine.onVideoStats = { [weak self] in self?.received($0) }
        engine.onAudioStats = { [weak self] in self?.audioStats = $0 }
        engine.onGamepadCombo = { [weak self] in self?.gamepadCombo() }
        engine.onHDR = { [weak self] screens in self?.sessionHDR = screens.contains(true) }
        engine.onMicrophone = { [weak self] in self?.microphoneChanged($0) }

        // Son : celui de la sortie (AirPods en audio spatial, ampli HDMI), ou resté sur l'ordinateur.
        let channels = settings.audio == .off ? 0 : (settings.audio == .auto ? AudioOutput.channels() : settings.audio.channels)
        var params = self.params(mode: mode, password: password, channels: channels)
        // Un visiteur n'a pas de presse-papiers (`Host.visitor`).
        params.clipboard = settings.clipboard
        params.vsync = settings.vsync
        screenHDR = HDRDisplay.isHDR(imageScreen)
        params.screenHDR = screenHDR
        // Les lumières de l'écran (automatiques ou réglées à l'œil), gardées jusqu'à la fin.
        params.screenLuminance = HDRCalibrationStore.session(.current(imageScreen), hdr: screenHDR)
        screenLuminance = params.screenLuminance
        params.hdrOffer = isVisitor ? nil : store.status[host.id]?.info?.hdr
        // Le micro retenu pour cet ordinateur, si le système le permet encore.
        params.microphone = settings.microphone && MicrophoneAccess.status == .granted
        if settings.microphone, MicrophoneAccess.status == .denied {
            micUI.denied()
            show(notice: MicText.permission, for: 6, opensSettings: true)
        }
        refreshMic()
        // « Stylet : Pression / Souris » retenu pour cet ordinateur.
        params.penMode = settings.pen
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
            // PyroWave (« Définition ramenée ») : le débit du lien, inconnu ici
            // (iOS ne donne ni la vitesse d'un adaptateur Ethernet ni celle du
            // Wi-Fi) ; la Station et le débit réglé plafonnent seuls.
            engine.setLinkMbps(nil)
        } catch {
            fail(error.localizedDescription)
        }
    }

    // MARK: - Écrans

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
        updateScreenHDR()
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
        // Face à une Station Mac, ⌘ part comme ⌘ (réglage ni montré ni appliqué).
        streamView.commandAsControl = host.commandAsControl
        streamView.mouseMode = host.settings.mouse.streamMode
        streamView.onTap = { [weak self] in self?.setBar(false) }
        streamView.onTopEdgeSwipe = { [weak self] in self?.setBar(true) }
        streamView.onStickyChanged = { [weak self] in self?.sticky = $0 }
        streamView.onPencil = { [weak self] in self?.pencilArrived() }
        // Clavier de l'iPad : ⌃⌥⌘P bascule « Stylet : Pression / Souris »,
        // avec l'avis bref (docs/link-ui.md, « Stylet »).
        streamView.shortcutLetters = ["p"]
        streamView.onShortcut = { [weak self] letter in
            guard let self, letter == "p" else { return }
            self.togglePenMode()
            self.show(notice: self.host.settings.pen.notice, for: 2)
        }
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
        ] + HDRDisplay.observe { [weak self] in
            MainActor.assumeIsolated { self?.updateScreenHDR() }
        } + [
            center.addObserver(forName: UIScene.willDeactivateNotification, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, self.isSessionScene(note.object), SessionRules.reactsToBackground(self.phase.stage) else { return }
                    self.streamView.releaseAll()
                    // Sur l'écran externe, l'image reste vivante : on la regarde
                    // encore, ce n'est pas l'arrière-plan (débit et image gardés).
                    if !self.external {
                        KyberLog.app.notice("fenêtre de la session inactive : image figée, débit réduit")
                        self.inBackground = true
                        self.engine?.setBackground(true)
                    }
                    self.show(notice: String(localized: "KyberNow Link continue en arrière-plan"), for: 4)
                }
            },
            center.addObserver(forName: UIScene.didActivateNotification, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, self.isSessionScene(note.object), self.inBackground else { return }
                    KyberLog.app.notice("fenêtre de la session active : image et débit repris")
                    self.inBackground = false
                    self.engine?.setBackground(false)
                }
            },
            center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.route = AudioRoute.current }
            },
            // Appel, Siri, alarme : à la fin de l'interruption, la session audio
            // repart et le micro allumé reprend sa capture.
            center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
                let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt
                MainActor.assumeIsolated { self?.audioInterrupted(raw.flatMap(AVAudioSession.InterruptionType.init(rawValue:))) }
            },
            center.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] note in
                MainActor.assumeIsolated {
                    guard let self, let c = note.object as? GCController, c.extendedGamepad != nil else { return }
                    // Face à un ordinateur qui n'en crée pas, l'avis des manettes indisponibles d'abord.
                    if !self.isVisitor, !self.host.gamepadsAvailable, !self.padsUnavailableTold { return }
                    self.show(notice: String(localized: "\(c.vendorName ?? String(localized: "Manette")) branchée"), for: 3)
                }
            },
        ]
    }

    private func audioInterrupted(_ type: AVAudioSession.InterruptionType?) {
        switch type {
        case .began:
            KyberLog.app.notice("son interrompu (appel, Siri, alarme)")
        case .ended:
            KyberLog.app.notice("fin de l'interruption du son : session audio reprise")
            try? AVAudioSession.sharedInstance().setActive(true)
            if micOn { engine?.microphoneSettingsChanged() }
        default:
            break
        }
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
        followCodec()
    }

    /// Les droits de l'invité ont changé (`SessionModel`) : le clavier et la
    /// souris retirés, la souris de l'iPad n'est plus capturée et le clavier
    /// de l'écran se replie ; les manettes retirées, la manette à l'écran se
    /// retire (elle n'envoie plus rien).
    override func rightsChanged(_ role: ShareRole) {
        streamView.mouseMode = role.keyboardAndMouse ? host.settings.mouse.streamMode : .desktop
        if !role.keyboardAndMouse, keyboardShown { toggleKeyboard() }
        if !role.pads, screenPad { toggleScreenPad() }
    }

    override var sharePanelOpen: Bool { shareSheet }
    override var lendPanelOpen: Bool { lendSheet }
    override func restoreOfferHint(_ offer: String) -> String { String(localized: " · « \(offer) » dans la barre") }
    override func openAutoTestAfterSession() { AppModel.shared?.autoTest.openAfterSession(host) }

    override func cameraOfferChanged(_ offered: Bool) {
        if offered { cameras = CameraCapture.cameras() }
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
        // La tuile « Stylet » : relue à l'ouverture de la barre.
        if open { refreshPen() }
        withAnimation(.spring(response: 0.32, dampingFraction: 0.86)) { barOpen = open }
        // Pupitre : tout l'écran reste le pavé tactile, barre ouverte ou non.
        streamView.touchesSuspended = open && !external
    }

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

    /// Le clavier de l'écran s'est replié : les touches collantes (Ctrl, Alt,
    /// ⊞) ne restent pas enfoncées sur l'ordinateur.
    func keyboardHidden() {
        keyboardShown = false
        streamView.releaseStickyModifiers()
    }

    func tap(_ usage: UInt16) { streamView.tap(usage: usage) }
    func toggleSticky(_ usage: UInt16) { streamView.toggleSticky(usage) }
    func chord(_ modifiers: [UInt16], _ usage: UInt16) { streamView.chord(modifiers, usage) }

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
        // HDR : l'écran de démonstration (`-hdrScreen YES` au simulateur), et un écran en HDR dès l'image.
        screenHDR = HDRDisplay.isHDR(imageScreen)
        sessionHDR = hdrOn
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
            // `-demoPyroWave wifi|relay` : l'image en PyroWave choisi à la main, en Wi-Fi ou par le relais
            // (bandeau du début, puce de la barre, « Images perdues » des statistiques).
            let pyrowave = UserDefaults.standard.string(forKey: "demoPyroWave")
            if let pyrowave {
                self.demoLink = pyrowave == "relay" ? .relay : .wifi
                if pyrowave == "relay" { self.path = RouteInfo(transport: .relay, relay: Relay.defaultServer, rttClient: 24, rttStation: 11) }
                self.host.settings.codec = .pyrowave
                self.receivedCodec = .pyrowave
            }
            for i in 0..<30 {
                var s = VideoStats()
                s.latencyMs = 8.4 + Double(i % 7) * 0.4
                s.fps = 120
                s.mbps = pyrowave == nil ? 46 : 290
                s.target = pyrowave == nil ? TargetBitrate(bitrate: 60_000_000, profile: 60_000_000, reason: 0)
                    : TargetBitrate(bitrate: 320_000_000, profile: 320_000_000, reason: 3)
                s.format = "4:2:0 8 bits"
                s.width = 2622
                s.height = 1206
                if pyrowave != nil, i % 9 == 4 { s.lostFrames = 2 }
                // `-demoLowered YES` : la définition ramenée (bandeau, ligne des statistiques).
                if pyrowave != nil, UserDefaults.standard.bool(forKey: "demoLowered") {
                    s.lowered = LoweredMode(width: 1920, height: 880, refresh: 60, mbps: 300)
                }
                self.received(s)
            }
            if pyrowave != nil, UserDefaults.standard.bool(forKey: "demoLowered") {
                self.loweredText = PyroWaveRules.loweredText(width: 1920, height: 880, refresh: 60, mbps: 300)
            }
            // `-demoStrained YES` : le bandeau du réseau qui sature à la place de celui du début.
            if pyrowave != nil { self.showPyroWaveBanner(strained: UserDefaults.standard.bool(forKey: "demoStrained")) }
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
    /// Démonstration : regarder un ordinateur du compte, deux écrans dans « Écran ».
    func demoWatch() {
        let screens = [ShareWatchScreen(demoID: 1, label: "Écran 1 — 3840 × 2160"), ShareWatchScreen(demoID: 2, label: "Écran 2 — 2560 × 1440")]
        guest = ShareFound(demoComputer: host.name, host: host.name, role: .spectator, watch: ShareWatch(demoScreens: screens))
        demoGuest(role: .spectator)
    }

    func demoGuest(role: ShareRole) {
        shareRole = role
        // La qualité reçue dans la barre : une image à part en 1080p · 60 Hz.
        shareImage = ShareImage.demo("1080p60", 1920, 1080, 60, follows: false)
        demo(.bar)
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

    #endif

    // MARK: - Fin

    /// Fin de session : le cœur rend la main à Admin (écrans et son de
    /// l'ordinateur rétablis, latence donnée à ses appareils).
    override func end() {
        guard phase != .ending && phase != .ended else { return }
        // « Annuler » pendant la préparation ou l'attente de l'hôte : la demande est retirée.
        if loan == nil { borrowFlow?.cancel() }
        GamepadBridge.shared.onMenuRequest = nil
        streamView.hideKeyboard()
        // Dernière image et statistiques (rien pour un visiteur).
        recordEnd(snapshotWidth: 1600, hdr: sessionHDR)
        withAnimation(.easeInOut(duration: 0.4)) {
            phase = .ending
            barOpen = false
        }
        tearDown()
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

    override func finish() {
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
        tearDown()
        withAnimation(Theme.easeOut) {
            barOpen = false
            phase = .failed(message)
        }
    }

    func dismissFailure() { finish() }

    /// Ce que `end()` et `fail()` défont tous les deux : écran externe, entrées,
    /// moteur, observateurs, et la rangée du clavier (qui tient ce contrôleur :
    /// sans la lâcher, chaque session restait en mémoire).
    private func tearDown() {
        leaveExternal()
        streamView.detach()
        streamView.keyboardAccessory = nil
        keyboardShown = false
        engine?.stop()
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        padsWatch = nil
    }
}
