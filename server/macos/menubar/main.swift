// KyberNow Station pour Mac : l'icône de la barre des menus (exécutable
// principal de « KyberNow Station.app », docs/station-macos.md, jalon 6).
// Pendant de l'icône de la zone de notification de Windows (server/tray) :
// état de la Station et de la session, Admin, partage, prêt, mise à jour,
// démarrage avec le Mac, autorisations, mot de passe d'Admin, désinstallation.
// Menu d'après la maquette validée par l'auteur (03/11/2026).
//
// Autres usages du même exécutable :
//   --permissions check|request …   autorisations (Permissions.swift)
//   --check-strings                  catalogues fr/en alignés

import AppKit

final class StatusController: NSObject, NSMenuDelegate {
    private let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    private let admin = AdminClient()
    private var snapshot = Snapshot()
    private let poller = DispatchQueue(label: "kybernow.poll")
    private var timer: Timer?
    /// Fenêtres de `kybernow-ui` ouvertes, par usage.
    private var windows: [String: pid_t] = [:]

    override init() {
        super.init()
        item.button?.image = statusIcon(.idle)
        item.button?.toolTip = "KyberNow Station"
        let menu = NSMenu()
        menu.delegate = self
        menu.autoenablesItems = false
        item.menu = menu
        refresh()
        timer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in self?.refresh() }
    }

    // MARK: État

    private func refresh() {
        poller.async { [weak self] in
            guard let self else { return }
            var s = Snapshot()
            s.kyber = serviceState(Labels.kyber)
            s.admin = serviceState(Labels.admin)
            s.state = s.admin == .running ? self.admin.call("GET", "/api/local/state") : nil
            s.startWithMac = startsWithMac()
            s.permissions = PermissionState.current()
            DispatchQueue.main.async {
                self.snapshot = s
                self.item.button?.image = statusIcon(s.kyber != .running || s.permissions.count < 3 ? .idle : s.inSession ? .session : .running)
                self.item.button?.toolTip = "KyberNow Station · \(self.statusLine(s).0)"
                if let menu = self.item.menu, menu.numberOfItems > 0 { self.build(menu) }
            }
        }
    }

    private func statusLine(_ s: Snapshot) -> (String, NSColor) {
        switch s.kyber {
        case .running where s.admin != .running || s.state == nil: return (t("status.adminDown"), .systemOrange)
        case .running where s.permissions.count < 3: return (t("status.permissions"), .systemOrange)
        case .running: return (t("status.running"), NSColor(srgbRed: 0x4C / 255, green: 0xD6 / 255, blue: 0x8A / 255, alpha: 1))
        case .starting: return (t("status.starting"), .systemOrange)
        default: return (t("status.stopped"), .tertiaryLabelColor)
        }
    }

    private func ago(_ seconds: Double) -> String {
        let minutes = Int((Date().timeIntervalSince1970 - seconds) / 60)
        if minutes < 60 { return t("ago.minutes", ["n": max(1, minutes)]) }
        let days = Calendar.current.dateComponents([.day], from: Calendar.current.startOfDay(for: Date(timeIntervalSince1970: seconds)), to: Calendar.current.startOfDay(for: Date())).day ?? 0
        return days == 0 ? t("ago.today") : days == 1 ? t("ago.yesterday") : t("ago.days", ["n": days])
    }

    /// Lignes sous l'état : session, mode, partage, prêt, demandes.
    private func details(_ s: Snapshot) -> [String] {
        var lines: [String] = []
        if let live = s.live, let name = live["name"] as? String {
            lines.append(t("session.connected", ["name": name]))
            if let mode = live["mode"] as? [String: Any], let w = mode["width"] as? Int, let h = mode["height"] as? Int, let hz = mode["refresh"] as? Int {
                lines.append(t("session.mode", ["w": w, "h": h, "hz": hz]))
            }
        } else if s.kyber == .running {
            if let last = s.last, let name = last["name"] as? String, let at = (last["ended_at"] ?? last["started_at"]) as? Double {
                lines.append(t("session.last", ["name": name, "when": ago(at)]))
            } else {
                lines.append(t("session.none"))
            }
        }
        if s.sharing { lines.append(t("session.sharing")) }
        if s.lent { lines.append(lendLine(s)) }
        if s.requests > 0 { lines.append(t("session.requests", ["n": s.requests])) }
        return lines
    }

    /// « Prêté à Léa jusqu'à 18 h », ou « Prêté à Léa · Reprise dans 1:42 »
    /// pendant le préavis (comme l'icône Windows, `lend::host::status_line`).
    private func lendLine(_ s: Snapshot) -> String {
        if let at = s.lendNotice {
            let left = max(0, Int(at - Date().timeIntervalSince1970))
            return t("lend.lentTo", ["name": s.borrower]) + " · " + t("lend.takingBackIn", ["left": String(format: "%d:%02d", left / 60, left % 60)])
        }
        if let until = s.lendUntil {
            let time = DateFormatter.localizedString(from: Date(timeIntervalSince1970: until), dateStyle: .none, timeStyle: .short)
            return t("lend.lentToUntil", ["name": s.borrower, "time": time])
        }
        return t("lend.lentTo", ["name": s.borrower])
    }

    // MARK: Menu

    func menuNeedsUpdate(_ menu: NSMenu) {
        build(menu)
        refresh()
    }

    private func header(_ s: Snapshot) -> NSMenuItem {
        let (status, color) = statusLine(s)
        let view = NSView(frame: NSRect(x: 0, y: 0, width: 300, height: 10))
        let dot = NSView(frame: NSRect(x: 14, y: 0, width: 8, height: 8))
        dot.wantsLayer = true
        dot.layer?.cornerRadius = 4
        dot.layer?.backgroundColor = color.cgColor
        if color != .tertiaryLabelColor {
            dot.layer?.shadowColor = color.cgColor
            dot.layer?.shadowOpacity = 0.8
            dot.layer?.shadowRadius = 3
            dot.layer?.shadowOffset = .zero
        }
        let title = NSTextField(labelWithString: status)
        title.font = .systemFont(ofSize: 13, weight: .semibold)
        let version = NSTextField(labelWithString: s.version)
        version.font = .monospacedSystemFont(ofSize: 10.5, weight: .regular)
        version.textColor = .secondaryLabelColor
        let lines = details(s)
        let sub = NSTextField(wrappingLabelWithString: lines.joined(separator: "\n"))
        sub.font = .systemFont(ofSize: 12)
        sub.textColor = .secondaryLabelColor
        sub.preferredMaxLayoutWidth = 252
        for v in [title, version, sub] { view.addSubview(v) }
        title.sizeToFit()
        version.sizeToFit()
        let subSize = lines.isEmpty ? .zero : sub.sizeThatFits(NSSize(width: 252, height: 400))
        let height = 10 + title.frame.height + (lines.isEmpty ? 0 : 4 + subSize.height) + 8
        view.frame.size.height = height
        title.frame.origin = NSPoint(x: 30, y: height - 10 - title.frame.height)
        dot.frame.origin = NSPoint(x: 16, y: title.frame.midY - 4)
        version.frame.origin = NSPoint(x: 300 - 14 - version.frame.width, y: title.frame.midY - version.frame.height / 2)
        sub.frame = NSRect(x: 30, y: 8, width: 252, height: subSize.height)
        view.addSubview(dot)
        let item = NSMenuItem()
        item.view = view
        return item
    }

    private func add(_ menu: NSMenu, _ title: String, _ action: Selector, enabled: Bool = true, key: String = "") -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        item.isEnabled = enabled
        menu.addItem(item)
        return item
    }

    /// Titre avec un complément aligné à droite (« 3 sur 3 », pastille de mise à jour).
    private func trailing(_ item: NSMenuItem, _ text: String, _ color: NSColor) {
        let style = NSMutableParagraphStyle()
        style.tabStops = [NSTextTab(textAlignment: .right, location: 250)]
        let title = NSMutableAttributedString(string: "\(item.title)\t", attributes: [.font: NSFont.menuFont(ofSize: 0), .paragraphStyle: style])
        title.append(NSAttributedString(string: text, attributes: [.font: NSFont.systemFont(ofSize: 11.5), .foregroundColor: color, .paragraphStyle: style]))
        item.attributedTitle = title
    }

    private func build(_ menu: NSMenu) {
        let s = snapshot
        menu.removeAllItems()
        menu.addItem(header(s))
        let running = s.kyber == .running
        let adminUp = s.state != nil
        if s.live != nil, adminUp { _ = add(menu, t("menu.end"), #selector(endSession)) }
        if s.requests > 0 { _ = add(menu, t("menu.requests"), #selector(openDevices)) }
        // Prêt : reprendre l'ordinateur, et les demandes d'emprunt.
        if s.lent, adminUp {
            _ = add(menu, s.lendNotice != nil ? t("menu.takeBackNow") : t("menu.takeBack"), #selector(takeBack))
        }
        for request in s.lendRequests {
            guard let id = request["id"] as? String, let name = request["name"] as? String else { continue }
            let item = NSMenuItem(title: t("lend.wants", ["name": name]), action: nil, keyEquivalent: "")
            let answers = NSMenu()
            for (title, accept) in [(t("lend.accept"), true), (t("lend.refuse"), false)] {
                let answer = NSMenuItem(title: title, action: #selector(answerRequest(_:)), keyEquivalent: "")
                answer.target = self
                answer.representedObject = ["id": id, "accept": accept] as [String: Any]
                answers.addItem(answer)
            }
            item.submenu = answers
            menu.addItem(item)
        }
        menu.addItem(.separator())
        _ = add(menu, t("menu.open"), #selector(openAdmin), enabled: s.admin != .missing)
        _ = add(menu, t("menu.share"), #selector(openShare), enabled: running && adminUp)
        _ = add(menu, t("menu.lend"), #selector(openLend), enabled: adminUp)
        menu.addItem(.separator())
        if let version = s.update {
            let item = add(menu, t("menu.update", ["version": version]), #selector(openUpdate))
            trailing(item, "●", NSColor(srgbRed: 0x9A / 255, green: 0x8C / 255, blue: 1, alpha: 1))
        } else {
            _ = add(menu, t("menu.updates"), #selector(openUpdate))
        }
        _ = add(menu, t("menu.restart"), #selector(restartStation), enabled: s.kyber != .missing)
        if running || s.kyber == .starting {
            _ = add(menu, t("menu.stop"), #selector(stopStation))
        } else {
            _ = add(menu, t("menu.start"), #selector(startStation), enabled: s.kyber != .missing)
        }
        let startup = add(menu, t("menu.startWithMac"), #selector(toggleStartWithMac))
        startup.state = s.startWithMac ? .on : .off
        menu.addItem(.separator())
        let permissions = add(menu, t("menu.permissions"), #selector(openPermissions))
        let n = s.permissions.count
        trailing(permissions, t("menu.permissionsCount", ["n": n]), n == 3 ? NSColor(srgbRed: 0x4C / 255, green: 0xD6 / 255, blue: 0x8A / 255, alpha: 1) : .systemOrange)
        _ = add(menu, t("menu.reset"), #selector(resetPassword), enabled: s.admin != .missing)
        _ = add(menu, t("menu.uninstall"), #selector(uninstall))
        menu.addItem(.separator())
        _ = add(menu, t("menu.quit"), #selector(quit), key: "q")
    }

    // MARK: Actions

    private func open(_ url: URL?) {
        if let url { NSWorkspace.shared.open(url) }
    }

    @objc func openAdmin() { open(adminURL()) }
    @objc func openShare() { open(adminURL("ensemble")) }
    @objc func openLend() { open(adminURL("pret")) }
    @objc func openDevices() { open(adminURL("appareils")) }

    @objc func takeBack() {
        let now = snapshot.lendNotice != nil
        poller.async { [weak self] in
            _ = self?.admin.call("POST", "/api/local/lend/take-back", body: ["now": now])
            DispatchQueue.main.async { self?.refresh() }
        }
    }

    @objc func answerRequest(_ sender: NSMenuItem) {
        guard let info = sender.representedObject as? [String: Any], let id = info["id"] as? String else { return }
        let accept = info["accept"] as? Bool ?? false
        poller.async { [weak self] in
            _ = self?.admin.call("POST", "/api/local/lend/requests/\(id)", body: ["accept": accept, "steam": "friend"])
            DispatchQueue.main.async { self?.refresh() }
        }
    }

    @objc func endSession() {
        poller.async { [weak self] in
            _ = self?.admin.call("POST", "/api/local/end")
            DispatchQueue.main.async { self?.refresh() }
        }
    }

    /// Fenêtre de `kybernow-ui` (mise à jour, désinstallation, autorisations),
    /// dans sa propre session de processus : elle survit à l'arrêt de l'icône
    /// (la désinstallation arrête ce service).
    private func window(_ mode: String) {
        if let pid = windows[mode], kill(pid, 0) == 0 {
            NSRunningApplication(processIdentifier: pid)?.activate()
            return
        }
        let exe = Paths.exe("kybernow-ui").path
        var attr: posix_spawnattr_t?
        posix_spawnattr_init(&attr)
        posix_spawnattr_setflags(&attr, Int16(POSIX_SPAWN_SETSID))
        var pid: pid_t = 0
        let args = [exe, "--mode", mode]
        var argv: [UnsafeMutablePointer<CChar>?] = args.map { strdup($0) } + [nil]
        if posix_spawn(&pid, exe, nil, &attr, &argv, environ) == 0 {
            windows[mode] = pid
            DispatchQueue.global().async { var status: Int32 = 0; waitpid(pid, &status, 0) }
        }
        posix_spawnattr_destroy(&attr)
        argv.forEach { free($0) }
    }

    @objc func openUpdate() { window("update") }
    @objc func openPermissions() { window("permissions") }
    @objc func uninstall() { window("uninstall") }

    private func act(_ work: @escaping () -> Void) {
        poller.async { [weak self] in
            work()
            DispatchQueue.main.async { self?.refresh() }
        }
    }

    @objc func restartStation() { act { restartService(Labels.kyber); restartService(Labels.admin) } }
    @objc func stopStation() { act { stopService(Labels.kyber) } }
    @objc func startStation() { act { startService(Labels.admin); startService(Labels.kyber) } }
    @objc func toggleStartWithMac() {
        let on = !snapshot.startWithMac
        act { setStartsWithMac(on) }
    }

    @objc func resetPassword() {
        let alert = NSAlert()
        alert.messageText = t("reset.title")
        alert.informativeText = t("reset.text")
        alert.addButton(withTitle: t("reset.ok"))
        alert.addButton(withTitle: t("reset.cancel"))
        NSApp.activate(ignoringOtherApps: true)
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        poller.async { [weak self] in
            let codeFile = Paths.admin.appendingPathComponent("setup-code.txt")
            try? FileManager.default.removeItem(at: codeFile)
            stopService(Labels.admin)
            let (status, _) = run(Paths.exe("kyber-admin").path, ["--data-dir", Paths.admin.path, "reset-password"])
            startService(Labels.admin)
            var code: String?
            for _ in 0..<40 where status == 0 {
                if let text = try? String(contentsOf: codeFile, encoding: .utf8), !text.isEmpty {
                    code = text.trimmingCharacters(in: .whitespacesAndNewlines)
                    break
                }
                Thread.sleep(forTimeInterval: 0.5)
            }
            DispatchQueue.main.async { self?.showCode(code) }
        }
    }

    private func showCode(_ code: String?) {
        let alert = NSAlert()
        NSApp.activate(ignoringOtherApps: true)
        guard let code else {
            alert.messageText = t("reset.failed")
            alert.runModal()
            return
        }
        alert.messageText = t("reset.code", ["code": code])
        alert.informativeText = t("reset.codeText")
        alert.addButton(withTitle: t("reset.openAdmin"))
        alert.addButton(withTitle: t("reset.copy"))
        switch alert.runModal() {
        case .alertFirstButtonReturn:
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(code, forType: .string)
            openAdmin()
        default:
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(code, forType: .string)
        }
    }

    @objc func quit() {
        let alert = NSAlert()
        alert.messageText = t("quit.title")
        alert.informativeText = t("quit.text")
        alert.addButton(withTitle: t("quit.keep"))
        alert.addButton(withTitle: t("quit.stop"))
        alert.addButton(withTitle: t("quit.cancel"))
        NSApp.activate(ignoringOtherApps: true)
        switch alert.runModal() {
        case .alertFirstButtonReturn: NSApp.terminate(nil)
        case .alertSecondButtonReturn:
            stopService(Labels.kyber)
            NSApp.terminate(nil)
        default: break
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    var controller: StatusController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        controller = StatusController()
    }

    /// Seconde ouverture (KyberNow Station dans Applications) : Admin.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        controller?.openAdmin()
        return false
    }
}

let arguments = Array(CommandLine.arguments.dropFirst())
if arguments.first == "--permissions" {
    exit(permissionsCommand(Array(arguments.dropFirst())))
}
if arguments.first == "--check-strings" {
    exit(checkStrings() ? 0 : 1)
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
