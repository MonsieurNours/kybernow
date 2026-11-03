// Une partie avec une console PlayStation sur l'Apple TV (textes :
// docs/link-ui.md, « Consoles PlayStation »). Même API que la classe du Mac
// (apps/macos/KyberNow/PlayStationSession.swift), que le magasin partagé
// (PlayStation.swift) crée et tient ; l'écran est celui d'une session
// d'ordinateur : l'image seule dans un GCEventViewController, le menu par le
// bouton Retour de la télécommande ou par Options + Create + L1 + R1 tenus
// 250 ms (la « barre de la partie » des autres apps).
//
// Les manettes vont au lecteur de manettes du cœur (`GamepadBridge`, mode
// console) : menu ouvert, le cœur envoie une manette au repos à la console,
// et la manette pilote KyberNow par le focus de tvOS. Le téléviseur passe en
// HDR10 quand la partie le demande (comme pour un ordinateur), à la
// fréquence des images demandée si « Caler le téléviseur sur la session »
// est actif. Agrandissement MetalFX en SDR, à partir de tvOS 27.1.

import AVKit
import Combine
import GameController
import KyberKit
import SwiftUI

@MainActor
final class PlayStationSession: ObservableObject {
    enum Phase: Equatable {
        case connecting, streaming
        /// Fin : le texte à montrer (nil : fermée ici).
        case ended(String?)
    }

    @Published private(set) var phase: Phase = .connecting
    /// Contenu protégé à l'écran de la console.
    @Published private(set) var cantDisplay = false
    @Published private(set) var menuOpen = false
    /// Les réglages de la console, ouverts depuis le menu.
    @Published var settingsOpen = false
    /// Mesures d'une seconde, et pertes depuis le début de la partie.
    @Published private(set) var stats: PlayStationPlay.Stats?
    @Published private(set) var lostTotal = 0
    @Published private(set) var repairedTotal = 0
    @Published private(set) var upscale: UpscaleReport?
    @Published private(set) var muted = false
    /// Par où passe la partie : puce « Par le relais », chemin des statistiques.
    @Published private(set) var path = RouteInfo.direct
    @Published private(set) var startedAt: Date?
    /// Rappel « Retour ou Options + Create + L1 + R1 : menu », un instant après l'image.
    @Published private(set) var menuHint = false

    let console: PlayStationConsole
    /// Images par seconde demandées (réglages).
    let requestedFps: Int
    /// HDR demandé à la console (puce « HDR » et téléviseur HDR).
    let hdr: Bool
    let streamView = StreamView(frame: CGRect(x: 0, y: 0, width: 1920, height: 1080))

    private let address: String
    private var remote: PlayStation.Remote?
    private let registered: String
    private let settings: String
    private var layout: String
    private let onEnd: (PlayStationSession, PlayStationPlay.End?) -> Void
    private var play: PlayStationPlay?
    private var end: PlayStationPlay.End?
    private var finished = false
    /// Mise en veille demandée ici : sa fin ne se dit pas.
    private var resting = false
    private var statsTask: Task<Void, Never>?
    private var displayCriteriaSet = false

    /// Mêmes paramètres que sur Mac ; `fullScreen` et `onFullScreen` n'ont pas
    /// cours sur un téléviseur (la partie est toujours en plein écran).
    init(console: PlayStationConsole, address: String, remote: PlayStation.Remote? = nil, registered: String, settings: String, layout: String, fullScreen: Bool,
         onFullScreen: @escaping (Bool) -> Void, onEnd: @escaping (PlayStationSession, PlayStationPlay.End?) -> Void) {
        self.console = console
        self.address = address
        self.remote = remote
        self.registered = registered
        self.settings = settings
        self.layout = layout
        self.onEnd = onEnd
        let s = try? JSONDecoder().decode(PSSettings.self, from: Data(settings.utf8))
        requestedFps = s?.fps ?? 60
        hdr = s?.hdr ?? false
        path = remote?.route ?? .direct
    }

    /// La partie à l'écran, puis la connexion.
    func open() {
        AppModel.current?.presentConsole(self)
        let pads = GamepadBridge.shared
        pads.attachConsole()
        pads.onMenuRequest = { [weak self] in self?.toggleMenu() }
        matchDisplay(true)

        let play = PlayStationPlay(view: streamView)
        play.onState = { [weak self] in self?.stateChanged($0) }
        play.onMenu = { [weak self] in self?.toggleMenu() }
        // Menu ouvert, la manette pilote KyberNow par le focus de tvOS : les
        // touches de navigation du cœur ne servent pas ici.
        play.onNavigate = { _ in }
        let s = try? JSONDecoder().decode(PSSettings.self, from: Data(settings.utf8))
        play.setUpscale(Upscaling.available ? s?.upscaleSettings : nil)
        self.play = play
        if !play.start(address: address, registered: registered, settings: settings,
                       hevcDecoder: DecoderCapabilities.current.hevc, layout: layout) {
            stateChanged(.ended(.notPaired))
        }
    }

    /// Déjà à l'écran : rien à faire de plus sur un téléviseur.
    func bringToFront() {
        AppModel.current?.presentConsole(self)
    }

    /// La partie a l'écran (le menu Session du Mac ; toujours vrai ici).
    var isKey: Bool { true }

    func setLayout(_ json: String) {
        layout = json
        play?.setLayout(json)
    }

    func setUpscale(_ u: UpscaleSettings?) {
        play?.setUpscale(Upscaling.available ? u : nil)
    }

    func setIntensities(rumble: Int, triggers: Int) {
        play?.setIntensities(rumble: rumble, triggers: triggers)
    }

    // MARK: Menu

    func toggleMenu() {
        if menuOpen { closeMenu() } else { openMenu() }
    }

    func openMenu() {
        guard phase == .streaming, !menuOpen else { return }
        menuHint = false
        play?.setCaptured(true)
        GamepadBridge.shared.setCaptured(true)
        withAnimation(Theme.easeOut) { menuOpen = true }
    }

    func closeMenu() {
        guard menuOpen else { return }
        settingsOpen = false
        play?.setCaptured(false)
        GamepadBridge.shared.setCaptured(false)
        withAnimation(Theme.easeOut) { menuOpen = false }
    }

    func toggleMute() {
        muted.toggle()
        play?.muted = muted
    }

    /// « Mettre la console en veille » : la partie se termine ensuite, sans message.
    func goToRest() {
        guard phase == .streaming else { return }
        resting = true
        closeMenu()
        play?.gotoBed()
    }

    var sessionMinutes: Int { startedAt.map { Int(Date().timeIntervalSince($0) / 60) } ?? 0 }

    // MARK: États

    private func stateChanged(_ s: PlayStationPlay.State) {
        switch s {
        case .connecting:
            phase = .connecting
        case let .streaming(w, h, hdr):
            KyberLog.app.notice("PlayStation : image \(w)x\(h) \(hdr ? "HDR" : "SDR", privacy: .public)")
            if phase != .streaming {
                startedAt = startedAt ?? Date()
                showMenuHint()
            }
            phase = .streaming
            startStats()
        case let .cantDisplay(on):
            cantDisplay = on
        case let .ended(e):
            guard !finished else { return }
            end = e
            cantDisplay = false
            closeMenu()
            statsTask?.cancel()
            statsTask = nil
            let text = resting ? nil : PSText.end(e)
            phase = .ended(text)
            stopPlay()
            if text == nil { close() }
        }
    }

    private func showMenuHint() {
        withAnimation(Theme.easeOut) { menuHint = true }
        DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
            withAnimation(Theme.easeOut) { self?.menuHint = false }
        }
    }

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
                if play.upscaleReport != self.upscale { self.upscale = play.upscaleReport }
                guard let s = play.stats() else { continue }
                self.lostTotal += s.lost
                self.repairedTotal += s.recovered
                self.stats = s
            }
        }
    }

    /// Le téléviseur en HDR10 si la partie le demande ; à la fréquence des
    /// images demandée avec « Caler le téléviseur sur la session » ; rendu à
    /// la fin (`on` faux).
    private func matchDisplay(_ on: Bool) {
        guard let window = UIApplication.shared.connectedScenes.compactMap({ ($0 as? UIWindowScene)?.keyWindow }).first else { return }
        let prefs = Preferences.shared
        guard on, prefs.matchFrameRate || hdr else {
            if displayCriteriaSet {
                window.avDisplayManager.preferredDisplayCriteria = nil
                displayCriteriaSet = false
            }
            return
        }
        let rate = prefs.matchFrameRate ? requestedFps : TVDisplay.native.refresh
        guard let criteria = HDRDisplay.criteria(refresh: rate, width: 1920, height: 1080, hdr: hdr) else { return }
        window.avDisplayManager.preferredDisplayCriteria = criteria
        displayCriteriaSet = true
        KyberLog.app.notice("téléviseur : \(rate) Hz\(self.hdr ? ", HDR10" : "", privacy: .public) demandés pour la console")
    }

    /// Arrêt de la partie hors du fil principal (`kn_ps_stop` attend sa fin).
    private func stopPlay() {
        guard let play else { return }
        self.play = nil
        let remote = self.remote
        self.remote = nil
        DispatchQueue.global(qos: .userInitiated).async {
            play.stop()
            withExtendedLifetime(remote) {}
        }
    }

    #if DEBUG
    /// Démonstration (`-screen consoleMenu`) : le menu ouvert, sans console.
    func demoMenu() {
        phase = .streaming
        startedAt = Date().addingTimeInterval(-23 * 60)
        stats = PlayStationPlay.Stats(width: 1920, height: 1080, hevc: true, hdr: hdr, targetKbps: 15_000, fps: 59.6, mbps: 12.4, lost: 0, recovered: 0)
        lostTotal = 3
        repairedTotal = 2
        upscale = UpscaleReport(state: .active(width: 3840, height: 2160), gpuMs: 0.9)
        menuOpen = true
    }
    #endif

    /// « Se déconnecter », « Fermer » après une fin, ou la fin voulue ici.
    func close() {
        guard !finished else { return }
        finished = true
        statsTask?.cancel()
        stopPlay()
        GamepadBridge.shared.onMenuRequest = nil
        GamepadBridge.shared.detachConsole()
        matchDisplay(false)
        streamView.detach()
        onEnd(self, end)
        AppModel.current?.consoleEnded(self)
    }
}
