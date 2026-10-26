// L'app hors de sa fenêtre (docs/link-ui.md, « Icône près de l'horloge ») :
// la fenêtre de l'accueil se cache au lieu de se fermer et l'app continue dans
// la barre des menus (MenuBar.swift) ; l'icône du Dock n'est là que tant
// qu'une fenêtre est ouverte ; « Ouvrir à l'ouverture de session » ; quitter
// termine d'abord une session en cours. Les commandes du menu et des
// notifications passent par les mêmes fonctions que l'accueil et les réglages.

import AppKit
import Combine
import KyberKit
import ServiceManagement
import SwiftUI
import UserNotifications

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var menuBar: MenuBarController?
    private var observers: [NSObjectProtocol] = []

    func applicationWillFinishLaunching(_ notification: Notification) {
        // Avant la fin du lancement : une notification qui a lancé l'app doit trouver son délégué.
        AppModel.shared.notifier.install(AppModel.shared)
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let app = AppModel.shared
        // Lancée à l'ouverture de session : sans fenêtre, dans l'icône seule.
        var hidden = LoginItem.launchedAtLogin()
        #if DEBUG
        if AppModel.demo, UserDefaults.standard.bool(forKey: "demoLoginLaunch") { hidden = true }
        #endif
        if hidden { app.startInBackground() }
        LoginItem.shared.applyDefault()
        menuBar = MenuBarController(app: app)
        app.watchStationConfirmations()
        // Icône du Dock et sondage : à chaque fenêtre qui s'ouvre, se ferme, se réduit.
        let names: [Notification.Name] = [
            NSWindow.didBecomeKeyNotification, NSWindow.didBecomeMainNotification, NSWindow.willCloseNotification,
            NSWindow.didMiniaturizeNotification, NSWindow.didDeminiaturizeNotification, NSWindow.didChangeOcclusionStateNotification,
        ]
        observers = names.map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { _ in
                // Après la notification : une fenêtre qui se ferme est encore visible pendant celle-ci.
                DispatchQueue.main.async { MainActor.assumeIsolated { app.windowsChanged() } }
            }
        }
        #if DEBUG
        if AppModel.demo, let menuBar { DemoRemote.install(app: app, menuBar: menuBar) }
        #endif
    }

    /// Rouvrir l'app (Finder, Launchpad, Spotlight, Dock) : l'accueil.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        AppModel.shared.showWindow()
        return false
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    /// « Quitter KyberNow Link », ⌘Q, le Dock : une session en cours se termine d'abord.
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        AppModel.shared.terminateReply()
    }
}

// MARK: - Fenêtre de l'accueil et Dock

/// Lit la fenêtre qui porte une vue SwiftUI.
struct WindowReader: NSViewRepresentable {
    let found: (NSWindow) -> Void

    final class Probe: NSView {
        var found: ((NSWindow) -> Void)?
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let window { found?(window) }
        }
    }

    func makeNSView(context: Context) -> Probe {
        let probe = Probe()
        probe.found = found
        return probe
    }

    func updateNSView(_ probe: Probe, context: Context) {
        probe.found = found
    }
}

/// Délégué de la fenêtre de l'accueil : fermer la cache (l'app continue dans
/// la barre des menus) ; le reste va au délégué de SwiftUI.
final class HomeWindowDelegate: NSObject, NSWindowDelegate {
    weak var original: NSWindowDelegate?
    var onClose: (() -> Void)?
    /// Une fois, à la sortie du plein écran (la fenêtre se cache ensuite).
    var onExitFullScreen: (() -> Void)?

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (original?.responds(to: selector) ?? false)
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        original?.responds(to: selector) == true ? original : nil
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        onClose?()
        return false
    }

    func windowDidExitFullScreen(_ notification: Notification) {
        let next = onExitFullScreen
        onExitFullScreen = nil
        next?()
        original?.windowDidExitFullScreen?(notification)
    }
}

@MainActor
enum Dock {
    /// Une fenêtre est ouverte : accueil (ou session), réglages de l'app, mise
    /// à jour de l'app ; réduite dans le Dock, elle compte aussi. Pas les menus,
    /// l'icône de la barre des menus ni les alertes.
    static var windowOpen: Bool {
        NSApp.windows.contains { w in
            w.styleMask.contains(.titled) && !(w is NSPanel) && w.alphaValue > 0 && (w.isVisible || w.isMiniaturized)
        }
    }

    /// Icône du Dock tant qu'une fenêtre est ouverte ; sans fenêtre, l'app ne
    /// vit que dans la barre des menus et rend le premier plan.
    static func update() {
        let policy: NSApplication.ActivationPolicy = windowOpen ? .regular : .accessory
        guard NSApp.activationPolicy() != policy else { return }
        NSApp.setActivationPolicy(policy)
        if policy == .accessory, NSApp.isActive { NSApp.deactivate() }
    }

    /// Avant d'ouvrir une fenêtre : l'icône du Dock d'abord, puis le premier
    /// plan (la fenêtre arrive devant, la barre des menus est celle de l'app).
    static func comeForward() {
        if NSApp.activationPolicy() != .regular { NSApp.setActivationPolicy(.regular) }
        NSApp.activate()
    }
}

extension AppModel {
    /// Fenêtre de l'accueil : son délégué la cache au lieu de la fermer.
    func attach(window: NSWindow) {
        if homeWindow !== window {
            homeWindow = window
            window.isReleasedWhenClosed = false
        }
        // La session pose son propre délégué le temps de la session, puis remet celui-ci.
        if !(window.delegate is HomeWindowDelegate), !(window.delegate is SessionWindowDelegate) {
            homeDelegate.original = window.delegate
            homeDelegate.onClose = { [weak self] in self?.closeWindow() }
            window.delegate = homeDelegate
        }
        if startHidden { hideAtLaunch(window) }
    }

    /// Lancée à l'ouverture de session : pas d'icône du Dock, la fenêtre de
    /// l'accueil cachée dès qu'elle existe.
    func startInBackground() {
        startHidden = true
        NSApp.setActivationPolicy(.accessory)
        if let homeWindow { hideAtLaunch(homeWindow) }
        if NSApp.isActive { NSApp.deactivate() }
        // Une fenêtre ouverte plus tard (menu, Finder) s'ouvre normalement.
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in
            guard let self, self.startHidden else { return }
            self.startHidden = false
            self.windowsChanged()
        }
    }

    private func hideAtLaunch(_ window: NSWindow) {
        // Transparente tout de suite (SwiftUI peut encore la mettre devant), puis retirée.
        window.alphaValue = 0
        DispatchQueue.main.async {
            window.orderOut(nil)
            window.alphaValue = 1
            self.windowsChanged()
        }
    }

    /// Une fenêtre s'est ouverte, fermée, réduite : icône du Dock, et
    /// sondage moins fréquent quand l'accueil n'est plus à l'écran.
    func windowsChanged() {
        guard !startHidden else { return }
        Dock.update()
        let shown = homeWindow.map { $0.isVisible || $0.isMiniaturized } ?? false
        store.pollInterval = shown ? 8 : 30
    }

    /// « Ouvrir KyberNow Link », rouvrir l'app, une notification : la fenêtre
    /// de l'accueil (ou de la session) au premier plan.
    func showWindow() {
        startHidden = false
        Dock.comeForward()
        guard let window = homeWindow else { return }
        if window.isMiniaturized { window.deminiaturize(nil) }
        window.makeKeyAndOrderFront(nil)
        windowsChanged()
    }

    /// Bouton rouge, ⌘W : la fenêtre se cache. Pendant une session, fermer la
    /// fenêtre, c'est se déconnecter : elle se cache une fois la session finie.
    func closeWindow() {
        guard let window = homeWindow else { return }
        if let session {
            hideAfterSession = true
            if session.isLive { session.end() } else { session.dismissFailure() }
            return
        }
        hide(window, exiting: false)
    }

    /// Après la session (la fenêtre a pu être fermée pendant) : `finish()` de
    /// la session sort du plein écran où elle avait mis la fenêtre ; un plein
    /// écran demandé à la main en session, on en sort ici.
    func sessionFinished(_ session: SessionController) {
        guard hideAfterSession, let homeWindow else { return }
        hideAfterSession = false
        let exiting = session.enteredFullScreen
        DispatchQueue.main.async { self.hide(homeWindow, exiting: exiting) }
    }

    /// Cache la fenêtre ; en plein écran, après en être sortie (`exiting` :
    /// la sortie est déjà lancée).
    private func hide(_ window: NSWindow, exiting: Bool) {
        if window.styleMask.contains(.fullScreen) {
            // Le délégué de la session remet le nôtre avant de sortir du plein écran.
            homeDelegate.onExitFullScreen = { [weak self] in self?.hide(window, exiting: false) }
            if !exiting { window.toggleFullScreen(nil) }
            return
        }
        window.orderOut(nil)
        windowsChanged()
        notifier.stillHere()
    }

    /// Fin de l'app : une session en cours se termine d'abord (Admin rend
    /// à l'ordinateur ses écrans et son son), 6 s au plus.
    func terminateReply() -> NSApplication.TerminateReply {
        // La session en cours, ou celle revenue à l'accueil dont Admin n'a pas encore fait le bilan.
        guard let pending = session.flatMap({ $0.isLive ? $0 : nil }) ?? lingering else { return .terminateNow }
        var replied = false
        let reply = {
            guard !replied else { return }
            replied = true
            NSApp.reply(toApplicationShouldTerminate: true)
        }
        afterSessionEnded = reply
        if pending.isLive { pending.end() }
        // Minuteur de la boucle (pas la file principale) : il tourne aussi quand la
        // demande de fin arrive depuis un bloc de la file principale, qui ne se vide
        // plus pendant l'attente.
        let timer = Timer(timeInterval: 6, repeats: false) { _ in MainActor.assumeIsolated { reply() } }
        RunLoop.main.add(timer, forMode: .common)
        RunLoop.main.add(timer, forMode: .modalPanel)
        return .terminateLater
    }

    /// La session est revenue à l'accueil : gardée jusqu'à sa fin dans le cœur.
    func linger(_ session: SessionController) {
        guard !session.closing.done else { return }
        lingering = session
        DispatchQueue.main.asyncAfter(deadline: .now() + 10) { [weak self, weak session] in
            if let self, self.lingering === session { self.lingering = nil }
        }
    }

    /// La session est finie dans le cœur : notification si elle s'est arrêtée
    /// sans « Se déconnecter », et la fin de l'app si elle l'attendait.
    func sessionEnded(_ session: SessionController) {
        if lingering === session { lingering = nil }
        if session.endedByComputer { notifier.sessionEnded(session.host, error: session.endError) }
        if let next = afterSessionEnded {
            afterSessionEnded = nil
            next()
        }
    }

    // MARK: - Commandes du menu et des notifications

    func perform(_ action: MenuAction) {
        switch action {
        case .disconnect: session?.end()
        case .fullScreen:
            showWindow()
            session?.toggleFullScreen()
        case let .mouse(choice):
            showWindow()
            session?.setMouse(choice)
        case .statistics:
            showWindow()
            session?.toggleStatsPanel()
        case let .connect(id): if let host = store.host(id) { connectFromOutside(host) }
        case let .wake(id): if let host = store.host(id) { store.wake(host) }
        case let .retry(id): if let host = store.host(id) { Task { await store.refresh(host) } }
        case let .profile(id, profile): if let host = store.host(id) { apply(profile, to: host) }
        case let .settings(id): if let host = store.host(id) { openSettingsFromOutside(host) }
        case let .admin(id): if let host = store.host(id) { openAdmin(host) }
        case let .restart(id): if let host = store.host(id) { stationFromOutside(.restart, host) }
        case let .update(id): if let host = store.host(id) { stationFromOutside(.update, host) }
        case .open: openHome(selecting: nil)
        case .add:
            showWindow()
            add()
        case .appSettings: openAppSettings()
        case .checkForUpdates:
            Dock.comeForward()
            checkForUpdates()
        case .quit: NSApp.terminate(nil)
        }
    }

    /// L'accueil, sur un ordinateur s'il est donné.
    func openHome(selecting host: Host?) {
        if session == nil {
            if let host { selectedID = host.id }
            route = .home
        }
        showWindow()
    }

    /// « Réglages… » d'un ordinateur, ou sa rubrique Station (notification).
    func openSettingsFromOutside(_ host: Host, section: ComputerSection? = nil) {
        if session == nil { openSettings(host, section: section) }
        showWindow()
    }

    /// « Se connecter », « Reprendre », « Reprendre la main » : l'accueil sur
    /// cet ordinateur, puis la session dans sa fenêtre. Une session qui finit
    /// encore (fermeture du passage) est attendue, 3 s au plus.
    func connectFromOutside(_ host: Host, attempt: Int = 0) {
        if let session {
            guard !session.isLive || session.phase == .ending, attempt < 30 else { return }
            if case .failed = session.phase { session.dismissFailure() }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { self.connectFromOutside(host, attempt: attempt + 1) }
            return
        }
        selectedID = host.id
        route = .home
        showWindow()
        // L'accueil se redessine sur cet ordinateur avant d'être photographié par le passage.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { self.connect(host) }
    }

    /// Profil choisi dans le menu : comme sur l'accueil ; en session sur cet
    /// ordinateur, tout de suite (comme le menu de la barre).
    func apply(_ profile: Profile, to host: Host) {
        if let session, session.isLive, session.host.id == host.id {
            session.select(profile)
            return
        }
        var h = host
        h.settings.apply(profile)
        store.update(h)
    }

    func openAdmin(_ host: Host) {
        #if DEBUG
        // Démonstration : pas de navigateur vers un ordinateur fictif.
        if Self.demo {
            KyberLog.app.notice("démonstration : Ouvrir Admin de \(host.name, privacy: .public)")
            return
        }
        #endif
        if let url = HomeAdmin.url(for: host, store: store) { NSWorkspace.shared.open(url) }
    }

    /// « Réglages de l'app… » : la scène Settings de SwiftUI, par la commande
    /// de son menu (KyberNow Link › Réglages…).
    func openAppSettings() {
        Dock.comeForward()
        guard let menu = NSApp.mainMenu?.items.first?.submenu,
              let index = menu.items.firstIndex(where: { $0.keyEquivalent == "," && $0.keyEquivalentModifierMask == .command }) else { return }
        menu.performActionForItem(at: index)
    }

    /// Redémarrer ou mettre à jour la Station hors de la fenêtre (menu, bouton
    /// d'une notification) : même suivi que la rubrique Station, et une
    /// notification à la fin.
    func stationFromOutside(_ kind: StationConfirm.Kind, _ host: Host) {
        if store.stationTasks[host.id]?.working == false { store.stationTasks[host.id] = nil }
        notifier.follow(host.id)
        if kind == .restart { store.restartStation(host) } else { store.updateStation(host) }
    }

    /// « {appareil} est connecté » : la rubrique Station la demande quand elle
    /// est à l'écran ; sinon (action lancée du menu ou d'une notification),
    /// une alerte de l'app, avec les mêmes textes.
    func watchStationConfirmations() {
        store.$stationConfirm
            .receive(on: DispatchQueue.main)
            .sink { [weak self] confirm in
                guard let self, let confirm, self.store.stationConfirm == confirm else { return }
                let shown = self.homeWindow?.isVisible == true && self.session == nil && self.route == .settings(confirm.hostID)
                // À l'écran, la rubrique Station la demande et montre la suite : pas de notification à la fin.
                if shown { self.notifier.unfollow(confirm.hostID) }
                guard !shown, self.notifier.following.contains(confirm.hostID) else { return }
                self.store.stationConfirm = nil
                self.confirmStation(confirm)
            }
            .store(in: &watchers)
    }

    private func confirmStation(_ confirm: StationConfirm) {
        guard let host = store.host(confirm.hostID) else { return }
        let alert = NSAlert()
        alert.messageText = confirm.title
        alert.informativeText = confirm.message
        alert.addButton(withTitle: confirm.confirmLabel).hasDestructiveAction = true
        alert.addButton(withTitle: StationText.cancel)
        NSApp.activate()
        if alert.runModal() == .alertFirstButtonReturn {
            if confirm.kind == .restart { store.restartStation(host, force: true) } else { store.updateStation(host, force: true) }
        } else {
            notifier.unfollow(confirm.hostID)
        }
    }
}

// MARK: - Ouverture à l'ouverture de session

/// « Ouvrir à l'ouverture de session » (Réglages de l'app) : l'app elle-même
/// comme élément d'ouverture (`SMAppService.mainApp`). Coché par défaut, une
/// fois, au premier lancement depuis un dossier Applications ; ensuite l'app
/// ne fait que refléter l'état (décoché dans Réglages Système, il le reste).
@MainActor
final class LoginItem: ObservableObject {
    static let shared = LoginItem()

    @Published private(set) var enabled = false
    /// Inscrite, mais à autoriser dans Réglages Système › Général › Ouverture.
    @Published private(set) var needsApproval = false

    /// La valeur par défaut a été appliquée (une seule fois).
    private static let defaultKey = "openAtLoginDefault"
    /// Démonstration : l'état, sans toucher aux éléments d'ouverture du Mac.
    private static let demoKey = "openAtLogin"

    private var defaults: UserDefaults { AppModel.shared.defaults }

    func refresh() {
        if AppModel.demo {
            enabled = defaults.object(forKey: Self.demoKey) as? Bool ?? false
            return
        }
        let status = SMAppService.mainApp.status
        enabled = status == .enabled
        needsApproval = status == .requiresApproval
    }

    func set(_ on: Bool) {
        if AppModel.demo {
            defaults.set(on, forKey: Self.demoKey)
        } else {
            do {
                if on { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
            } catch {
                KyberLog.app.error("ouverture à l'ouverture de session : \(error.localizedDescription, privacy: .public)")
            }
        }
        refresh()
    }

    /// Premier lancement de la version qui apporte le réglage : coché.
    func applyDefault() {
        guard defaults.object(forKey: Self.defaultKey) == nil else { return refresh() }
        if AppModel.demo {
            defaults.set(true, forKey: Self.defaultKey)
            defaults.set(true, forKey: Self.demoKey)
            return refresh()
        }
        // Un build de développement (DerivedData) ne s'inscrit pas.
        guard Bundle.main.bundleURL.deletingLastPathComponent().lastPathComponent == "Applications" else { return refresh() }
        defaults.set(true, forKey: Self.defaultKey)
        if SMAppService.mainApp.status != .enabled { set(true) } else { refresh() }
    }

    /// L'app a été lancée par l'ouverture de session (événement d'ouverture
    /// marqué « élément d'ouverture » ; lu dans `applicationDidFinishLaunching`).
    static func launchedAtLogin() -> Bool {
        guard let event = NSAppleEventManager.shared().currentAppleEvent else { return false }
        return event.eventID == kAEOpenApplication
            && event.paramDescriptor(forKeyword: keyAEPropData)?.enumCodeValue == keyAELaunchedAsLogInItem
    }
}

#if DEBUG
// MARK: - Démonstration

/// Démonstration : l'app se pilote de l'extérieur, pour vérifier le menu, la
/// fenêtre, le Dock et les notifications sans souris. Commandes en
/// notification distribuée « fr.leslignesclaires.kybernow.demo », l'objet
/// étant la commande (`menu-open:6`, `dump:/chemin.txt`, `window-close`…,
/// voir `run`). `-demoJournal <fichier>` : les notifications et les états
/// demandés y sont écrits.
@MainActor
enum DemoRemote {
    private static var app: AppModel?
    private static var menuBar: MenuBarController?
    /// Apparence imposée aux menus dessinés (`appearance:dark|light|auto`).
    private static var appearance: NSAppearance?
    /// Menu ouvert par une commande, refermé par son minuteur.
    private static var tracked: NSMenu?
    private static var observer: NSObjectProtocol?

    static func install(app: AppModel, menuBar: MenuBarController) {
        self.app = app
        self.menuBar = menuBar
        observer = DistributedNotificationCenter.default().addObserver(forName: Notification.Name("fr.leslignesclaires.kybernow.demo"), object: nil, queue: .main) { note in
            guard let command = note.object as? String else { return }
            // Hors du bloc de la file principale : un menu ouvert ou une fin de l'app
            // attendue doivent laisser cette file se vider, comme après un vrai clic.
            RunLoop.main.perform(inModes: [.default]) { MainActor.assumeIsolated { run(command) } }
        }
        journal("prêt")
    }

    nonisolated static func journal(_ line: String) {
        guard let path = UserDefaults.standard.string(forKey: "demoJournal") else { return }
        let text = "\(Date().formatted(.iso8601.time(includingFractionalSeconds: true))) \(line)\n"
        if let handle = FileHandle(forWritingAtPath: path) {
            handle.seekToEndOfFile()
            handle.write(Data(text.utf8))
            try? handle.close()
        } else {
            try? text.write(toFile: path, atomically: true, encoding: .utf8)
        }
    }

    private static func host(_ index: String) -> Host? {
        guard let app, let i = Int(index), app.store.ordered.indices.contains(i) else { return nil }
        return app.store.ordered[i]
    }

    /// Ferme le menu (ou le sous-menu) au bout de `seconds`, avec des touches
    /// envoyées en chemin (`down`, `right`…) pour ouvrir un sous-menu ; juste
    /// avant, les fenêtres des menus ouverts dessinées dans `shots` (PNG),
    /// ce qui marche aussi écran verrouillé.
    private static func track(_ menu: NSMenu, seconds: Double, keys: [String], shots: String? = nil) {
        if let shots {
            // Deux dessins quand le menu reste ouvert assez longtemps : au début et à la fin (mesures en direct).
            var moments = [(max(0.5, seconds - 0.4), shots)]
            if seconds >= 3 { moments.append((1.0, shots + "-debut")) }
            for (delay, prefix) in moments {
                let t = Timer(timeInterval: delay, repeats: false) { _ in
                    MainActor.assumeIsolated { snapshotMenus(to: prefix) }
                }
                RunLoop.main.add(t, forMode: .common)
            }
        }
        for (i, key) in keys.enumerated() {
            let t = Timer(timeInterval: 0.6 + Double(i) * 0.35, repeats: false) { _ in
                let codes: [String: (UInt16, Int)] = ["down": (125, NSDownArrowFunctionKey), "up": (126, NSUpArrowFunctionKey),
                                                       "right": (124, NSRightArrowFunctionKey), "left": (123, NSLeftArrowFunctionKey)]
                guard let (code, char) = codes[key], let scalar = UnicodeScalar(char),
                      let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                   windowNumber: 0, context: nil, characters: String(Character(scalar)),
                                                   charactersIgnoringModifiers: String(Character(scalar)), isARepeat: false, keyCode: code) else { return }
                NSApp.postEvent(event, atStart: false)
            }
            RunLoop.main.add(t, forMode: .common)
        }
        tracked = menu
        let close = Timer(timeInterval: seconds, repeats: false) { _ in MainActor.assumeIsolated { tracked?.cancelTracking() } }
        RunLoop.main.add(close, forMode: .common)
    }

    /// Les fenêtres de menu ouvertes, de gauche à droite : `<préfixe>-1.png`, `-2.png`…
    private static func snapshotMenus(to prefix: String) {
        let windows = NSApp.windows.filter { $0.isVisible && String(describing: type(of: $0)).contains("Menu") }
            .sorted { $0.frame.minX < $1.frame.minX }
        journal("fenêtres de menu : \(windows.map { "\(type(of: $0)) \(Int($0.frame.width))x\(Int($0.frame.height))" })")
        for (i, window) in windows.enumerated() {
            guard let view = window.contentView?.superview ?? window.contentView,
                  let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { continue }
            view.cacheDisplay(in: view.bounds, to: rep)
            try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "\(prefix)-\(i + 1).png"))
        }
    }

    static func run(_ command: String) {
        guard let app, let menuBar else { return }
        let parts = command.split(separator: ":", maxSplits: 3, omittingEmptySubsequences: false).map(String.init)
        journal("commande \(command)")
        switch parts[0] {
        case "menu-open":
            // menu-open:<secondes>[:touches,…[:<préfixe des PNG>]]
            let seconds = parts.count > 1 ? Double(parts[1]) ?? 5 : 5
            let keys = parts.count > 2 ? parts[2].split(separator: ",").map(String.init) : []
            let shots = parts.count > 3 ? parts[3] : nil
            menuBar.menu.appearance = appearance
            track(menuBar.menu, seconds: seconds, keys: keys, shots: shots)
            menuBar.item.button?.performClick(nil)
        case "submenu":
            // submenu:<ordinateur>[/<ligne du sous-menu>]:<secondes>[:<préfixe des PNG>] : le sous-menu seul, sous l'icône.
            menuBar.render()
            let path = parts.count > 1 ? parts[1].split(separator: "/").compactMap { Int($0) } : [0]
            let seconds = parts.count > 2 ? Double(parts[2]) ?? 5 : 5
            let offset = app.session?.isLive == true ? 7 : 0
            guard let first = path.first, menuBar.menu.items.indices.contains(first + offset),
                  var sub = menuBar.menu.items[first + offset].submenu else { return }
            for i in path.dropFirst() {
                guard sub.items.indices.contains(i), let next = sub.items[i].submenu else { return }
                sub = next
            }
            let frame = menuBar.item.button?.window?.frame ?? .zero
            let shots = parts.count > 3 ? parts[3] : nil
            sub.appearance = appearance
            track(sub, seconds: seconds, keys: [], shots: shots)
            sub.popUp(positioning: nil, at: NSPoint(x: frame.minX, y: frame.minY - 4), in: nil)
        case "dump":
            menuBar.render()
            let text = MenuContent.text(MenuContent.entries(app)) + "\n"
            try? text.write(toFile: parts[1], atomically: true, encoding: .utf8)
        case "icons":
            // icons:<dossier> : l'icône éteinte et allumée, à 1x et 2x.
            for (name, lit) in [("off", false), ("lit", true)] {
                let image = MenuBarIcon.image(lit: lit)
                for scale in [1, 2] {
                    let px = 18 * scale
                    guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8, samplesPerPixel: 4,
                                                     hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else { continue }
                    rep.size = image.size
                    NSGraphicsContext.saveGraphicsState()
                    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
                    image.draw(in: NSRect(origin: .zero, size: image.size))
                    NSGraphicsContext.restoreGraphicsState()
                    try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "\(parts[1])/icon-\(name)@\(scale)x.png"))
                }
            }
        case "state":
            let frame = menuBar.item.button?.window?.frame ?? .zero
            let top = (NSScreen.screens.first?.frame.height ?? 0) - frame.maxY
            let policy = switch NSApp.activationPolicy() { case .regular: "regular"; case .accessory: "accessory"; default: "prohibited" }
            let lines = [
                "politique \(policy)", "active \(NSApp.isActive)", "fenêtre visible \(app.homeWindow?.isVisible == true)",
                "fenêtres \(NSApp.windows.filter { $0.isVisible && $0.styleMask.contains(.titled) }.map { $0.title.isEmpty ? "(sans titre)" : $0.title })",
                "premier plan \(NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? "?")",
                "sondage \(Int(app.store.pollInterval)) s", "session \(app.session.map { "\($0.host.name) \($0.phase)" } ?? "aucune")",
                "icône \(Int(frame.minX)),\(Int(top)) \(Int(frame.width))x\(Int(frame.height))",
                "ouverture de session \(LoginItem.shared.enabled)",
            ]
            journal("état : " + lines.joined(separator: " | "))
            if parts.count > 1 { try? (lines.joined(separator: "\n") + "\n").write(toFile: parts[1], atomically: true, encoding: .utf8) }
        case "button":
            // button:<préfixe> : le bouton de l'icône dessiné en clair et en sombre (« -clair », « -sombre »).
            guard let button = menuBar.item.button else { return }
            let original = button.appearance
            for (name, appearance) in [("clair", NSAppearance.Name.aqua), ("sombre", .darkAqua)] {
                button.appearance = NSAppearance(named: appearance)
                button.display()
                if let rep = button.bitmapImageRepForCachingDisplay(in: button.bounds) {
                    button.cacheDisplay(in: button.bounds, to: rep)
                    try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "\(parts[1])-\(name).png"))
                }
            }
            button.appearance = original
        case "windows-shot":
            // windows-shot:<préfixe> : chaque fenêtre ouverte de l'app, dessinée (« -1 », « -2 »…).
            let windows = NSApp.windows.filter { $0.isVisible && $0.styleMask.contains(.titled) }
            journal("fenêtres dessinées : \(windows.map(\.title))")
            for (i, window) in windows.enumerated() {
                guard let view = window.contentView?.superview ?? window.contentView,
                      let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { continue }
                view.cacheDisplay(in: view.bounds, to: rep)
                try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "\(parts[1])-\(i + 1).png"))
            }
        case "appearance":
            appearance = parts.count > 1 && parts[1] == "dark" ? NSAppearance(named: .darkAqua)
                : parts.count > 1 && parts[1] == "light" ? NSAppearance(named: .aqua) : nil
        case "window-close": app.closeWindow()
        case "window-open": app.showWindow()
        case "quit": NSApp.terminate(nil)
        case "perform":
            // perform:<ligne>[/<ligne>…] : comme un clic sur cette ligne du menu.
            menuBar.render()
            var menu: NSMenu = menuBar.menu
            let path = parts.count > 1 ? parts[1].split(separator: "/").compactMap { Int($0) } : []
            for (n, i) in path.enumerated() {
                guard menu.items.indices.contains(i) else { return }
                if n == path.count - 1 {
                    let item = menu.items[i]
                    guard item.isEnabled, let action = item.action else { journal("ligne inactive"); return }
                    NSApp.sendAction(action, to: item.target, from: item)
                } else if let sub = menu.items[i].submenu {
                    menu = sub
                }
            }
        case "ended":
            // ended:<raison> (vide : Admin ou un autre appareil) : la session finie par l'ordinateur.
            app.session?.demoEndedByComputer(error: parts.count > 1 && !parts[1].isEmpty ? parts[1] : nil)
        case "status":
            // status:<ordinateur>:online|offline|unknown|searching
            guard let h = host(parts[1]) else { return }
            let info = app.store.status[h.id]?.info
            switch parts[2] {
            case "online": app.store.setDemoStatus(HostStatus(online: true, info: info), for: h.id)
            case "offline": app.store.setDemoStatus(HostStatus(online: false, info: info), for: h.id)
            // Hors ligne et sans réponse possible : le réveil n'aboutira pas.
            case "unknown": app.store.setDemoStatus(HostStatus(online: false, info: nil), for: h.id)
            default: app.store.setDemoStatus(nil, for: h.id)
            }
        case "nomac":
            // nomac:<ordinateur> : l'app ne connaît pas son adresse MAC.
            guard var h = host(parts[1]) else { return }
            h.wake = nil
            app.store.update(h)
        case "custom":
            // custom:<ordinateur> : réglages écartés du profil (« Personnalisé »).
            guard var h = host(parts[1]) else { return }
            h.settings.profile = nil
            app.store.update(h)
        case "station":
            // station:<ordinateur>:update|setup|current
            guard let h = host(parts[1]), let info = app.store.status[h.id]?.info else { return }
            let station = switch parts[2] {
            case "update": StationInfo(version: "0.5.0", updateAvailable: true, updateVersion: "0.5.1", installable: true)
            case "setup": StationInfo(version: "0.5.0", updateAvailable: true, updateVersion: "0.6.0", requiresSetup: true)
            default: StationInfo(version: "0.5.0")
            }
            app.store.setDemoStatus(HostStatus(online: app.store.status[h.id]?.online ?? true,
                                               info: HostInfo(hostname: info.hostname, gpu: info.gpu, av1: info.av1, virtualDisplay: info.virtualDisplay,
                                                              speakers: info.speakers, os: info.os, adminVersion: station.version, station: station)), for: h.id)
        case "respond":
            // respond:<genre>:<ordinateur>:<bouton|default>
            let h = parts.count > 2 ? host(parts[2]) : nil
            let action = parts.count > 3 && parts[3] != "default" ? parts[3] : UNNotificationDefaultActionIdentifier
            app.notifier.respond(kind: parts[1], host: h?.id.uuidString ?? "", action: action)
        case "delivered":
            UNUserNotificationCenter.current().getDeliveredNotifications { list in
                let text = list.map { "\($0.request.identifier) | \($0.request.content.title) | \($0.request.content.body)" }
                journal("livrées : \(text.count)\n" + text.joined(separator: "\n"))
            }
        case "auth":
            UNUserNotificationCenter.current().getNotificationSettings { s in
                journal("autorisation \(s.authorizationStatus.rawValue) (0 indéterminée, 1 refusée, 2 accordée, 3 provisoire)")
            }
        default:
            journal("commande inconnue")
        }
    }
}
#endif
