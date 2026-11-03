// Une partie avec une console PlayStation, dans sa fenêtre (textes :
// docs/link-ui.md, « Consoles PlayStation »). L'image passe par la même vue
// et le même rendu qu'une session d'ordinateur (StreamView, VideoToolbox,
// Metal, HDR sur un écran HDR) ; le protocole est dans le cœur
// (`PlayStationPlay`). La fenêtre s'ouvre aux deux tiers de l'écran, en
// 16:9, ou en plein écran (celui de macOS) selon les réglages de la console ;
// le plein écran rejoint ou quitté pendant la partie y est retenu.
//
// La barre de la partie (PlayStationBar.swift) est celle d'une session
// d'ordinateur : elle s'ouvre au pointeur contre le bord haut (souris libre)
// ou par Options + Create + L1 + R1 ; tant qu'elle est ouverte, la console
// reçoit une manette au repos et ni clavier ni souris.

import AppKit
import GameController
import KyberKit
import SwiftUI

@MainActor
final class PlayStationSession: NSObject, ObservableObject, NSWindowDelegate {
    enum Phase: Equatable {
        case connecting, streaming
        /// Fin : le texte à montrer (nil : fermée par ce Mac).
        case ended(String?)
    }

    @Published private(set) var phase: Phase = .connecting
    /// Contenu protégé à l'écran de la console.
    @Published private(set) var cantDisplay = false

    let console: PlayStationConsole
    private let address: String
    private let registered: String
    /// Réglages de la console (JSON du cœur, HDR compris) et disposition du clavier et de la souris.
    private let settings: String
    private var layout: String
    private let fullScreen: Bool
    private let onFullScreen: (Bool) -> Void
    private let onEnd: (PlayStationSession, PlayStationPlay.End?) -> Void
    private let streamView = StreamView(frame: .zero)
    /// Clavier et souris (au-dessus de l'image, sous les textes).
    private let input = PlayStationInputView(frame: .zero)
    /// Souris capturée : aide « ⌃⌥⌘M » montrée un instant, sinon l'invitation à cliquer.
    @Published private(set) var mouseCaptured = false
    @Published private(set) var showReleaseHint = false
    /// La console a ouvert son clavier : les touches y écrivent (bandeau en bas).
    @Published private(set) var consoleKeyboard = false
    private var hints: NSHostingView<PlayStationHints>?
    private var play: PlayStationPlay?
    private var window: NSWindow?
    private var overlay: NSHostingView<PlayStationOverlay>?
    private var end: PlayStationPlay.End?
    private var finished = false
    /// Mise en veille demandée ici : sa fin ne se dit pas.
    private var resting = false

    // MARK: Barre de la partie

    /// Ce que la manette parcourt dans la barre, dans l'ordre.
    enum BarItem: Equatable { case stats, sound, fullScreen, mouse, settings, rest, disconnect }

    @Published private(set) var barVisible = false
    /// La manette pilote la barre (ouverte par la combinaison, ou touchée barre ouverte).
    @Published private(set) var padMode = false
    @Published private(set) var padFocus = 0
    /// Panneau des mesures, en bas à droite.
    @Published private(set) var statsPanel = false
    /// Dernières mesures (une fois par seconde) et pertes depuis le début de la partie.
    @Published private(set) var stats: PlayStationPlay.Stats?
    @Published private(set) var lostTotal = 0
    @Published private(set) var repairedTotal = 0
    @Published private(set) var muted = false
    @Published private(set) var isFullScreen = false
    /// Une manette branchée : pas de « Prendre la souris » ni d'invitation à cliquer.
    @Published private(set) var padConnected = !GCController.controllers().isEmpty
    /// Images par seconde demandées à la console (réglages).
    let requestedFps: Int
    /// Rectangle de la barre et bouton de fermeture du panneau, dans la vue
    /// de la partie (origine en haut à gauche), mesurés par SwiftUI.
    var barRect: CGRect = .zero
    var statsCloseRect: CGRect = .zero
    private var chrome: PlayStationChromeView?
    private var openBar: Task<Void, Never>?
    private var watchBar: Task<Void, Never>?
    private var statsTask: Task<Void, Never>?
    private var hideChrome: Task<Void, Never>?
    private var observers: [NSObjectProtocol] = []
    /// Le pointeur reste contre le bord haut ce temps-là pour ouvrir la barre.
    private static let barOpenDelay: Duration = .milliseconds(250)
    /// La barre se referme ce temps après que le pointeur l'a quittée.
    private static let barCloseDelay: TimeInterval = 0.4
    /// « Contre le bord haut » : à 2 points au plus.
    private static let barEdge: CGFloat = 2
    private static let barMargin: CGFloat = 12
    /// Largeur de la barre avant sa première mesure.
    private static let barEstimatedWidth: CGFloat = 900

    var barItems: [BarItem] {
        // L'ordre de la barre, de gauche à droite.
        var items: [BarItem] = [.stats, .sound]
        if !padConnected { items.append(.mouse) }
        return items + [.settings, .fullScreen, .rest, .disconnect]
    }

    /// Bande du haut où la barre prend les clics (plus haute avec le bandeau manette).
    var barHeight: CGFloat {
        guard !barRect.isEmpty else { return padMode ? 150 : 96 }
        return barRect.maxY + (padMode ? 84 : 30)
    }

    init(console: PlayStationConsole, address: String, registered: String, settings: String, layout: String, fullScreen: Bool,
         onFullScreen: @escaping (Bool) -> Void, onEnd: @escaping (PlayStationSession, PlayStationPlay.End?) -> Void) {
        self.console = console
        self.address = address
        self.registered = registered
        self.settings = settings
        self.layout = layout
        self.fullScreen = fullScreen
        self.onFullScreen = onFullScreen
        self.onEnd = onEnd
        requestedFps = (try? JSONDecoder().decode(PSSettings.self, from: Data(settings.utf8)))?.fps ?? 60
    }

    func open() {
        let screen = NSApp.keyWindow?.screen ?? NSScreen.main ?? NSScreen.screens[0]
        let area = screen.visibleFrame
        var w = area.width * 2 / 3, h = w * 9 / 16
        if h > area.height * 0.85 { h = area.height * 0.85; w = h * 16 / 9 }
        let content = NSRect(x: area.midX - w / 2, y: area.midY - h / 2, width: w, height: h)
        let window = NSWindow(contentRect: content, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.title = console.name
        window.appearance = NSAppearance(named: .darkAqua)
        window.backgroundColor = .black
        window.contentAspectRatio = NSSize(width: 16, height: 9)
        window.collectionBehavior = [.fullScreenPrimary, .managed]
        window.tabbingMode = .disallowed
        window.isRestorable = false
        window.isReleasedWhenClosed = false
        let container = NSView(frame: NSRect(origin: .zero, size: content.size))
        container.autoresizingMask = [.width, .height]
        streamView.frame = container.bounds
        streamView.autoresizingMask = [.width, .height]
        container.addSubview(streamView)
        input.frame = container.bounds
        input.autoresizingMask = [.width, .height]
        input.onCapture = { [weak self] captured in self?.captureChanged(captured) }
        container.addSubview(input)
        let hints = NSHostingView(rootView: PlayStationHints(session: self))
        hints.frame = container.bounds
        hints.autoresizingMask = [.width, .height]
        container.addSubview(hints)
        self.hints = hints
        let chrome = PlayStationChromeView(rootView: PlayStationChrome(session: self))
        chrome.session = self
        chrome.frame = container.bounds
        chrome.autoresizingMask = [.width, .height]
        chrome.isHidden = true
        container.addSubview(chrome)
        self.chrome = chrome
        input.onPointerMoved = { [weak self] spot in self?.pointerMoved(spot) }
        input.onShortcut = { [weak self] key in self?.shortcut(key) ?? false }
        let overlay = NSHostingView(rootView: PlayStationOverlay(session: self))
        overlay.frame = container.bounds
        overlay.autoresizingMask = [.width, .height]
        container.addSubview(overlay)
        self.overlay = overlay
        window.contentView = container
        window.delegate = self
        self.window = window
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(input)
        if fullScreen { window.toggleFullScreen(nil) }

        let play = PlayStationPlay(view: streamView)
        input.play = play
        play.onState = { [weak self] in self?.stateChanged($0) }
        play.onMenu = { [weak self] in self?.padCombo() }
        play.onNavigate = { [weak self] in self?.padNavigate($0) }
        play.onKeyboard = { [weak self] in self?.keyboardEvent($0) }
        input.onConsoleKeyboardDone = { [weak self] in self?.consoleKeyboard = false }
        play.setUpscale((try? JSONDecoder().decode(PSSettings.self, from: Data(settings.utf8)))?.upscaleSettings)
        observe()
        self.play = play
        if !play.start(address: address, registered: registered, settings: settings,
                       hevcDecoder: DecoderCapabilities.current.hevc, layout: layout) {
            stateChanged(.ended(.notPaired))
        }
    }

    /// Disposition changée dans les réglages : tout de suite dans la partie.
    func setLayout(_ json: String) {
        layout = json
        play?.setLayout(json)
    }

    /// Agrandissement changé dans les réglages : tout de suite dans la partie.
    func setUpscale(_ u: UpscaleSettings?) {
        play?.setUpscale(u)
    }

    /// Intensités de la manette changées dans les réglages : tout de suite.
    func setIntensities(rumble: Int, triggers: Int) {
        play?.setIntensities(rumble: rumble, triggers: triggers)
    }

    /// Le clavier de la console : ouvert, texte renvoyé par la console, fermé.
    private func keyboardEvent(_ e: PlayStationPlay.KeyboardEvent) {
        switch e {
        case let .open(text):
            KyberLog.app.notice("PlayStation : clavier de la console ouvert")
            input.beginConsoleKeyboard(text: text)
            consoleKeyboard = true
        case let .text(text):
            input.setConsoleText(text)
        case .closed:
            KyberLog.app.notice("PlayStation : clavier de la console fermé")
            input.endConsoleKeyboard()
            consoleKeyboard = false
        }
    }

    func bringToFront() {
        NSApp.activate()
        window?.makeKeyAndOrderFront(nil)
    }

    func goToRest() {
        guard case .streaming = phase else { return }
        setBar(false)
        resting = true
        play?.gotoBed()
    }

    func toggleMute() {
        muted.toggle()
        play?.muted = muted
    }

    func toggleFullScreen() {
        window?.toggleFullScreen(nil)
    }

    /// « Prendre la souris » / « Rendre la souris » (⌃⌥⌘M).
    func toggleMouse() {
        setBar(false)
        if mouseCaptured { input.releaseAll() } else { input.capture() }
    }

    func toggleStatsPanel() {
        guard phase == .streaming else { return }
        withAnimation(Theme.easeOut) { statsPanel.toggle() }
        if !statsPanel { statsCloseRect = .zero }
        updateChrome()
    }

    /// « Réglages de {nom} » : l'accueil passe devant, aux réglages de la console.
    func openConsoleSettings() {
        setBar(false)
        AppModel.shared.openSettings(console)
        AppModel.shared.showWindow()
    }

    /// La fenêtre de la partie a le clavier (commandes du menu Session).
    var isKey: Bool { window?.isKeyWindow == true }

    /// ⌃⌥⌘D, ⌃⌥⌘F, ⌃⌥⌘S : ceux du menu Session, pour cette partie.
    private func shortcut(_ key: String) -> Bool {
        switch key {
        case "d": close()
        case "f": toggleFullScreen()
        case "s": toggleStatsPanel()
        default: return false
        }
        return true
    }

    // MARK: - Barre au pointeur

    /// Contre le bord haut de l'image, dans la largeur de la barre, centrée.
    private func inOpenZone(_ spot: CGPoint) -> Bool {
        let width = barRect.width > 0 ? barRect.width : Self.barEstimatedWidth
        return spot.y <= Self.barEdge && abs(spot.x - input.bounds.width / 2) <= width / 2
    }

    /// Sur la barre, à 12 points près, ou entre elle et le bord haut.
    private func keepsBarOpen(_ spot: CGPoint) -> Bool {
        let width = barRect.width > 0 ? barRect.width : Self.barEstimatedWidth
        let rect = barRect.isEmpty ? CGRect(x: (input.bounds.width - width) / 2, y: 0, width: width, height: 96) : barRect
        return spot.x >= rect.minX - Self.barMargin && spot.x <= rect.maxX + Self.barMargin && spot.y <= rect.maxY + Self.barMargin
    }

    /// La barre peut s'ouvrir au pointeur : partie en cours, app au premier
    /// plan, fenêtre clé, souris libre, aucun bouton tenu.
    private var pointerCanOpen: Bool {
        phase == .streaming && !barVisible && NSApp.isActive && window?.isKeyWindow == true && !mouseCaptured
    }

    /// Le pointeur libre a bougé : resté un quart de seconde contre le bord
    /// haut, il ouvre la barre (un passage rapide ne l'ouvre pas).
    private func pointerMoved(_ spot: CGPoint?) {
        guard pointerCanOpen, let spot, inOpenZone(spot), NSEvent.pressedMouseButtons == 0 else {
            openBar?.cancel()
            openBar = nil
            return
        }
        guard openBar == nil else { return }
        openBar = Task { [weak self] in
            try? await Task.sleep(for: Self.barOpenDelay)
            guard !Task.isCancelled, let self else { return }
            self.openBar = nil
            guard self.pointerCanOpen, NSEvent.pressedMouseButtons == 0, let spot = self.input.pointerSpot, self.inOpenZone(spot) else { return }
            self.setBar(true)
        }
    }

    /// Barre ouverte au pointeur : elle se referme 0,4 s après qu'il l'a
    /// quittée (regardé dix fois par seconde : un pointeur sur ses boutons
    /// n'envoie plus de mouvement à l'image).
    private func watchPointer() {
        watchBar?.cancel()
        watchBar = Task { [weak self] in
            var awaySince: Date?
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(100))
                guard let self, !Task.isCancelled, self.barVisible else { return }
                if self.padMode { awaySince = nil; continue }
                if let spot = self.input.pointerSpot, self.keepsBarOpen(spot) {
                    awaySince = nil
                } else if let since = awaySince {
                    if Date().timeIntervalSince(since) >= Self.barCloseDelay { self.setBar(false) }
                } else {
                    awaySince = Date()
                }
            }
        }
    }

    /// Ouvre ou ferme la barre (`pad` : ouverte par la manette, qui la pilote).
    /// Ouverte, la souris est rendue et la console ne reçoit plus rien.
    func setBar(_ visible: Bool, pad: Bool = false) {
        openBar?.cancel()
        openBar = nil
        if visible {
            guard phase == .streaming, !barVisible else { return }
            if mouseCaptured { input.releaseAll() }
            input.suspended = true
            play?.setCaptured(true)
            padFocus = 0
            padMode = pad
            withAnimation(.spring(response: 0.32, dampingFraction: 0.86)) { barVisible = true }
            watchPointer()
        } else {
            guard barVisible else { return }
            watchBar?.cancel()
            watchBar = nil
            withAnimation(.easeOut(duration: 0.2)) { padMode = false }
            withAnimation(.spring(response: 0.32, dampingFraction: 0.86)) { barVisible = false }
            input.suspended = false
            play?.setCaptured(false)
            window?.makeFirstResponder(input)
        }
        updateChrome()
    }

    /// La couche de la barre n'est dans la fenêtre que si elle a quelque chose
    /// à montrer (l'image va ainsi directement à l'écran) ; le trait de repère
    /// s'allume là où le geste qui ouvre la barre est possible.
    private func updateChrome() {
        let needed = barVisible || statsPanel
        hideChrome?.cancel()
        hideChrome = nil
        if needed {
            chrome?.isHidden = false
        } else if chrome?.isHidden == false {
            // Le temps que la barre s'en aille.
            hideChrome = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(400))
                guard let self, !Task.isCancelled, !self.barVisible, !self.statsPanel else { return }
                self.chrome?.isHidden = true
            }
        }
        play?.setTopMarker(phase == .streaming && !barVisible && NSApp.isActive && window?.isKeyWindow == true && !mouseCaptured)
    }

    // MARK: - Manette

    /// Options + Create + L1 + R1 : la barre s'ouvre et la manette la pilote,
    /// ou se ferme.
    private func padCombo() {
        guard phase == .streaming else { return }
        if barVisible {
            setBar(false)
        } else {
            setBar(true, pad: true)
        }
    }

    private func padNavigate(_ key: PadKey) {
        guard barVisible else { return }
        // Barre ouverte au pointeur, la manette la prend.
        if !padMode, key != .back {
            padFocus = 0
            withAnimation(.easeOut(duration: 0.2)) { padMode = true }
            updateChrome()
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

    func activate(_ item: BarItem) {
        switch item {
        case .stats: toggleStatsPanel()
        case .sound: toggleMute()
        case .fullScreen: toggleFullScreen()
        case .mouse: toggleMouse()
        case .settings: openConsoleSettings()
        case .rest: goToRest()
        case .disconnect: close()
        }
    }

    // MARK: - Mesures

    /// Une fois par seconde pendant la partie : les mesures, et les pertes
    /// additionnées depuis le début.
    private func startStats() {
        guard statsTask == nil else { return }
        statsTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard let self, !Task.isCancelled, let play = self.play else { return }
                guard let s = play.stats() else { continue }
                self.lostTotal += s.lost
                self.repairedTotal += s.recovered
                self.stats = s
            }
        }
    }

    /// App au premier plan ou non, manettes branchées ou débranchées.
    private func observe() {
        let center = NotificationCenter.default
        for name in [NSApplication.didBecomeActiveNotification, NSApplication.didResignActiveNotification] {
            observers.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.updateChrome() }
            })
        }
        for name in [Notification.Name.GCControllerDidConnect, .GCControllerDidDisconnect] {
            observers.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated {
                    guard let self else { return }
                    self.padConnected = !GCController.controllers().isEmpty
                    self.padFocus = min(self.padFocus, self.barItems.count - 1)
                }
            })
        }
    }

    private func stateChanged(_ s: PlayStationPlay.State) {
        switch s {
        case .connecting:
            phase = .connecting
        case let .streaming(w, h, hdr):
            KyberLog.app.notice("PlayStation : image \(w)x\(h) \(hdr ? "HDR" : "SDR", privacy: .public)")
            phase = .streaming
            startStats()
        case let .cantDisplay(on):
            cantDisplay = on
        case let .ended(e):
            guard !finished else { return }
            end = e
            cantDisplay = false
            input.endConsoleKeyboard()
            consoleKeyboard = false
            setBar(false)
            statsPanel = false
            statsTask?.cancel()
            statsTask = nil
            // Fin voulue ici (fenêtre fermée) ou mise en veille : la fenêtre se ferme.
            let text = resting ? nil : PSText.end(e)
            phase = .ended(text)
            stopPlay()
            if text == nil { close() }
        }
        overlay?.isHidden = phase == .streaming && !cantDisplay
        updateChrome()
    }

    /// Arrêt de la partie hors du fil principal (`kn_ps_stop` attend sa fin).
    private func stopPlay() {
        guard let play else { return }
        self.play = nil
        DispatchQueue.global(qos: .userInitiated).async { play.stop() }
    }

    private func captureChanged(_ captured: Bool) {
        mouseCaptured = captured
        showReleaseHint = captured
        // Un clic dans l'image (souris prise) ferme la barre.
        if captured { setBar(false) }
        updateChrome()
        if captured {
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in self?.showReleaseHint = false }
        }
    }

    func close() {
        guard !finished else { return }
        finished = true
        openBar?.cancel()
        watchBar?.cancel()
        statsTask?.cancel()
        hideChrome?.cancel()
        observers.forEach { NotificationCenter.default.removeObserver($0) }
        observers = []
        input.onPointerMoved = nil
        input.onShortcut = nil
        input.releaseAll()
        input.play = nil
        stopPlay()
        streamView.detach()
        window?.delegate = nil
        window?.orderOut(nil)
        window?.close()
        window = nil
        onEnd(self, end)
    }

    // MARK: NSWindowDelegate

    /// Plein écran sans barre des menus ni Dock, comme une session
    /// d'ordinateur : le haut de l'écran est à la barre de la partie (sinon
    /// la barre des menus et la barre de titre descendent et la cachent).
    nonisolated func window(_ window: NSWindow, willUseFullScreenPresentationOptions proposedOptions: NSApplication.PresentationOptions = []) -> NSApplication.PresentationOptions {
        [.fullScreen, .hideDock, .hideMenuBar]
    }

    /// La fenêtre perd le clavier : souris rendue, tout relâché.
    nonisolated func windowDidResignKey(_ notification: Notification) {
        MainActor.assumeIsolated {
            input.releaseAll()
            updateChrome()
        }
    }

    nonisolated func windowDidBecomeKey(_ notification: Notification) {
        MainActor.assumeIsolated { updateChrome() }
    }

    nonisolated func windowDidEnterFullScreen(_ notification: Notification) {
        MainActor.assumeIsolated {
            isFullScreen = true
            onFullScreen(true)
        }
    }

    nonisolated func windowDidExitFullScreen(_ notification: Notification) {
        MainActor.assumeIsolated {
            isFullScreen = false
            onFullScreen(false)
        }
    }

    nonisolated func windowShouldClose(_ sender: NSWindow) -> Bool {
        MainActor.assumeIsolated { close() }
        return false
    }
}

/// Aides du clavier et de la souris, en bas de l'image pendant la partie :
/// l'invitation à cliquer tant que la souris n'est pas capturée, puis
/// « ⌃⌥⌘M » un instant après la capture. Ne prend pas les clics.
struct PlayStationHints: View {
    @ObservedObject var session: PlayStationSession

    var body: some View {
        VStack {
            Spacer()
            if session.phase == .streaming, let text = text {
                Text(text)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.white)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background(.black.opacity(0.6), in: Capsule())
                    .padding(.bottom, 24)
                    .transition(.opacity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .allowsHitTesting(false)
        .animation(.easeInOut(duration: 0.2), value: text)
    }

    private var text: String? {
        // Le clavier de la console l'emporte : il dit ce que font les touches.
        if session.consoleKeyboard { return PSText.consoleKeyboard }
        if session.showReleaseHint { return PSText.releaseMouse }
        // Une manette branchée l'emporte sur le clavier et la souris.
        return session.mouseCaptured || session.padConnected ? nil : PSText.clickToPlay
    }
}

/// Ce qui se dit par-dessus l'image : la connexion, le contenu protégé, la fin.
struct PlayStationOverlay: View {
    @ObservedObject var session: PlayStationSession

    var body: some View {
        ZStack {
            switch session.phase {
            case .connecting:
                ZStack {
                    Theme.background
                    HStack(spacing: 12) {
                        ProgressView().controlSize(.small)
                        Text(PSText.connecting).font(Theme.sans(16)).foregroundStyle(Theme.text2)
                    }
                }
            case .streaming:
                if session.cantDisplay {
                    Text(PSText.cantDisplay)
                        .font(Theme.sans(16)).foregroundStyle(Theme.text)
                        .padding(.horizontal, 20).padding(.vertical, 14)
                        .glass(12, tint: 0.6)
                }
            case let .ended(text):
                ZStack {
                    Theme.background
                    VStack(spacing: 18) {
                        if let text {
                            Text(text).font(Theme.sans(17)).foregroundStyle(Theme.text)
                                .multilineTextAlignment(.center)
                                .frame(maxWidth: 520)
                        }
                        Button(String(localized: "Fermer")) { session.close() }
                            .buttonStyle(GhostButton(height: 40))
                            .keyboardShortcut(.defaultAction)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .preferredColorScheme(.dark)
    }
}
