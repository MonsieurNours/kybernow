// Icône de la barre des menus (docs/link-ui.md, « Icône près de l'horloge ») :
// le symbole de KyberNow Link en image modèle (le système le teinte), allumé
// en session ; un clic ouvre le menu. En session : l'en-tête, les mesures en
// direct, Plein écran, Souris, Statistiques, Se déconnecter ; puis chaque
// ordinateur, dans l'ordre de l'accueil, avec son sous-menu ; puis la fenêtre,
// l'ajout, les réglages de l'app, les mises à jour, Quitter.
//
// `NSStatusItem` et `NSMenu` plutôt que `MenuBarExtra` : le menu est
// reconstruit à chaque ouverture (les ordinateurs sont sondés tout de suite),
// et mis à jour sur place chaque seconde tant qu'il est ouvert (mesures, état
// des ordinateurs), ce que le menu de SwiftUI ne permet pas.

import AppKit
import Combine
import KyberKit
import SwiftUI

/// Ce que fait une ligne du menu (AppModel.perform, AppLife.swift).
enum MenuAction: Equatable {
    case disconnect, fullScreen, mouse, statistics
    case connect(UUID), wake(UUID), retry(UUID), profile(UUID, Profile), settings(UUID), admin(UUID), restart(UUID), update(UUID)
    case open, add, appSettings, checkForUpdates, quit
}

/// Point d'état devant un ordinateur (couleurs d'état de brand/v2, `kybernow.css`).
enum StatusDot: String, Equatable {
    /// En ligne : `--kn-ok`.
    case ok
    /// Hors ligne : `--kn-bad`.
    case bad
    /// Réveil en cours : `--kn-warn`.
    case warn
    /// Recherche : `--kn-faint`.
    case faint

    /// Version sombre (menu sombre) et claire.
    private var colors: (dark: UInt32, light: UInt32) {
        switch self {
        case .ok: return (0x4CD68A, 0x138A4E)
        case .bad: return (0xFF6B6B, 0xC93A3A)
        case .warn: return (0xF5B841, 0xA86A00)
        case .faint: return (0x65737E, 0x74828D)
        }
    }

    private static func color(_ rgb: UInt32) -> NSColor {
        NSColor(srgbRed: CGFloat(rgb >> 16 & 0xFF) / 255, green: CGFloat(rgb >> 8 & 0xFF) / 255, blue: CGFloat(rgb & 0xFF) / 255, alpha: 1)
    }

    private static var cache: [StatusDot: NSImage] = [:]

    /// Point de 8 points, dessiné à chaque affichage selon l'apparence du
    /// menu (pas une image modèle : il garde sa couleur).
    @MainActor
    var image: NSImage {
        if let image = Self.cache[self] { return image }
        let colors = colors
        let image = NSImage(size: NSSize(width: 8, height: 8), flipped: false) { rect in
            let dark = NSAppearance.currentDrawing().bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            Self.color(dark ? colors.dark : colors.light).setFill()
            NSBezierPath(ovalIn: rect).fill()
            return true
        }
        image.isTemplate = false
        Self.cache[self] = image
        return image
    }
}

/// Une ligne du menu, tirée de l'état de l'app.
struct MenuEntry: Equatable {
    var title = ""
    var enabled = true
    var checked = false
    var action: MenuAction?
    var children: [MenuEntry]?
    var separator = false
    var key = ""
    /// Point d'état (ordinateurs) ; le texte de l'état reste dans le titre.
    var dot: StatusDot?

    static let line = MenuEntry(separator: true)
}

/// Le contenu du menu, dans l'ordre du contrat.
@MainActor
enum MenuContent {
    static func entries(_ app: AppModel) -> [MenuEntry] {
        let store = app.store
        let session = app.session.flatMap { $0.isLive ? $0 : nil }
        var out: [MenuEntry] = []
        if let session {
            out.append(MenuEntry(title: String(localized: "En session sur \(session.host.name)"), enabled: false))
            out.append(MenuEntry(title: metrics(session), enabled: false))
            out.append(MenuEntry(title: String(localized: "Plein écran"), checked: app.homeWindow?.styleMask.contains(.fullScreen) == true, action: .fullScreen))
            out.append(MenuEntry(title: String(localized: "Souris des jeux ou du bureau"), action: .mouse))
            out.append(MenuEntry(title: String(localized: "Statistiques"), action: .statistics))
            out.append(MenuEntry(title: String(localized: "Se déconnecter"), action: .disconnect))
            out.append(.line)
        }
        let hosts = store.ordered
        for host in hosts {
            out.append(computer(host, store: store, session: session))
        }
        if !hosts.isEmpty { out.append(.line) }
        out.append(MenuEntry(title: String(localized: "Ouvrir KyberNow Link"), action: .open))
        // Pendant une session, la fenêtre est à la session : l'ajout et les réglages d'un ordinateur attendent.
        out.append(MenuEntry(title: String(localized: "Ajouter un ordinateur…"), enabled: session == nil, action: .add))
        out.append(MenuEntry(title: String(localized: "Réglages de l'app…"), action: .appSettings))
        out.append(MenuEntry(title: String(localized: "Rechercher des mises à jour…"), action: .checkForUpdates))
        out.append(.line)
        out.append(MenuEntry(title: String(localized: "Quitter KyberNow Link"), action: .quit, key: "q"))
        return out
    }

    /// « 12,4 ms · 60 i/s · 80 Mbit/s » : la latence de la barre de session,
    /// les images par seconde, le débit ; avant l'image, l'étape de la connexion.
    static func metrics(_ session: SessionController) -> String {
        if let stats = session.stats {
            var parts: [String] = []
            if let ms = stats.latencyMs { parts.append(Moment.ms(ms)) }
            if stats.fps > 0 { parts.append(String(localized: "\(Int(stats.fps.rounded())) i/s")) }
            if stats.mbps > 0 { parts.append("\(Int(stats.mbps.rounded())) Mbit/s") }
            if !parts.isEmpty { return parts.joined(separator: " · ") }
        }
        switch session.phase {
        case .preparing: return String(localized: "Préparation de l'écran")
        case .connecting: return session.stationRestarting ? StationText.restarting : String(localized: "Connexion")
        case .starting: return String(localized: "Premières images")
        case .reconnecting: return String(localized: "Reconnexion à \(session.host.name)…")
        default: return "—"
        }
    }

    /// « {nom} — En ligne » et son sous-menu.
    static func computer(_ host: Host, store: HostStore, session: SessionController?) -> MenuEntry {
        let status = store.status[host.id]
        let online = status?.online
        let inSession = session?.host.id == host.id
        var items: [MenuEntry] = []

        // La Station : comme la rubrique Station (LinkUI.swift).
        let station = status?.info?.station
        let task = store.stationTasks[host.id]
        var restarting = station?.restarting == true
        var updating = station?.installing == true
        if case .restarting? = task { restarting = true }
        if case .updating? = task { updating = true }

        if inSession {
            items.append(MenuEntry(title: String(localized: "Se déconnecter"), action: .disconnect))
        } else if online == false, restarting || updating {
            // La Station revient : ni réveil ni nouvel essai, la connexion attend son retour.
            items.append(MenuEntry(title: String(localized: "Se connecter (\(host.settings.profileLabel))"), enabled: false))
        } else if online == false {
            if store.waking.contains(host.id) {
                items.append(MenuEntry(title: LinkText.waking, enabled: false))
            } else if store.canWake(host) {
                items.append(MenuEntry(title: LinkText.wake, action: .wake(host.id)))
            } else {
                items.append(MenuEntry(title: String(localized: "Réessayer"), action: .retry(host.id)))
            }
        } else {
            // Comme le bouton de l'accueil (aussi pendant « Recherche… ») ; une session à la fois.
            items.append(MenuEntry(title: String(localized: "Se connecter (\(host.settings.profileLabel))"), enabled: session == nil, action: .connect(host.id)))
        }

        let current = host.settings.profile
        var profiles = Profile.allCases.map { MenuEntry(title: $0.label, checked: $0 == current, action: .profile(host.id, $0)) }
        if current == nil { profiles.append(MenuEntry(title: Profile.label(nil), enabled: false, checked: true)) }
        items.append(.line)
        items.append(MenuEntry(title: String(localized: "Profil"), children: profiles))
        items.append(MenuEntry(title: String(localized: "Réglages…"), enabled: session == nil, action: .settings(host.id)))

        // Admin, redémarrer, mettre à jour.
        var admin: [MenuEntry] = []
        if online == true { admin.append(MenuEntry(title: StationText.openAdmin, action: .admin(host.id))) }
        if restarting {
            admin.append(MenuEntry(title: StationText.restarting, enabled: false))
        } else if updating || (online == true && station != nil) {
            admin.append(MenuEntry(title: StationText.restart, enabled: !updating, action: .restart(host.id)))
        }
        if updating {
            admin.append(MenuEntry(title: StationText.updating, enabled: false))
        } else if online == true, let station, station.updateAvailable, station.installable, let v = station.updateVersion {
            admin.append(MenuEntry(title: String(localized: "Mettre à jour la Station (\(v))"), enabled: !restarting, action: .update(host.id)))
        }
        if !admin.isEmpty {
            items.append(.line)
            items += admin
        }
        let dot: StatusDot = store.waking.contains(host.id) || restarting || updating ? .warn : online == true ? .ok : online == false ? .bad : .faint
        return MenuEntry(title: "\(host.name) — \(LinkText.status(online))", checked: inSession, children: items, dot: dot)
    }

    /// Le menu en texte, pour le vérifier (démonstration) : ✓ coché, ▸ sous-menu, [inactif].
    static func text(_ entries: [MenuEntry], depth: Int = 0) -> String {
        entries.map { e -> String in
            let pad = String(repeating: "    ", count: depth)
            if e.separator { return pad + "────────" }
            var line = pad + (e.checked ? "✓ " : "  ") + (e.dot.map { "(\($0.rawValue)) " } ?? "") + e.title
            if e.children != nil { line += "  ▸" }
            if !e.enabled { line += "  [inactif]" }
            if !e.key.isEmpty { line += "  ⌘\(e.key.uppercased())" }
            if let c = e.children { line += "\n" + text(c, depth: depth + 1) }
            return line
        }.joined(separator: "\n")
    }
}

// MARK: - Icône et menu

@MainActor
final class MenuBarController: NSObject, NSMenuDelegate {
    private let app: AppModel
    let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    let menu = NSMenu()
    /// Mise à jour du menu ouvert, chaque seconde.
    private var timer: Timer?
    private var watchers = Set<AnyCancellable>()
    private var phaseWatch: AnyCancellable?
    private let icons = (off: MenuBarIcon.image(lit: false), lit: MenuBarIcon.image(lit: true))

    final class ActionBox: NSObject {
        let action: MenuAction
        init(_ action: MenuAction) { self.action = action }
    }

    init(app: AppModel) {
        self.app = app
        super.init()
        menu.delegate = self
        menu.autoenablesItems = false
        item.menu = menu
        item.button?.imagePosition = .imageOnly
        updateIcon()
        // L'icône s'allume pendant une session.
        app.$session.receive(on: DispatchQueue.main).sink { [weak self] session in
            self?.phaseWatch = session?.$phase.receive(on: DispatchQueue.main).sink { _ in self?.updateIcon() }
            self?.updateIcon()
        }.store(in: &watchers)
    }

    private func updateIcon() {
        let session = app.session.flatMap { $0.isLive ? $0 : nil }
        item.button?.image = session == nil ? icons.off : icons.lit
        let tip = session.map { String(localized: "KyberNow Link · en session sur \($0.host.name)") } ?? "KyberNow Link"
        item.button?.toolTip = tip
        item.button?.setAccessibilityLabel(tip)
    }

    // MARK: Menu

    func menuNeedsUpdate(_ menu: NSMenu) {
        render()
    }

    /// À l'ouverture : les ordinateurs sondés tout de suite, puis le menu suivi chaque seconde.
    func menuWillOpen(_ menu: NSMenu) {
        Task { await app.store.refreshAll() }
        timer?.invalidate()
        let timer = Timer(timeInterval: 1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.render() }
        }
        // Mode commun : le minuteur tourne pendant que le menu est ouvert.
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    func menuDidClose(_ menu: NSMenu) {
        timer?.invalidate()
        timer = nil
    }

    func render() {
        apply(MenuContent.entries(app), to: menu)
    }

    /// Même forme (mêmes lignes, mêmes sous-menus) : mise à jour sur place, le
    /// menu ouvert ne bouge pas ; sinon, reconstruit.
    private func apply(_ entries: [MenuEntry], to menu: NSMenu) {
        guard Self.sameShape(entries, menu.items) else {
            menu.removeAllItems()
            for e in entries { menu.addItem(makeItem(e)) }
            return
        }
        for (e, it) in zip(entries, menu.items) where !e.separator {
            configure(it, e)
            if let children = e.children, let sub = it.submenu { apply(children, to: sub) }
        }
    }

    private static func sameShape(_ entries: [MenuEntry], _ items: [NSMenuItem]) -> Bool {
        guard entries.count == items.count else { return false }
        for (e, it) in zip(entries, items) {
            if e.separator != it.isSeparatorItem || (e.children != nil) != (it.submenu != nil) { return false }
            if let children = e.children, let sub = it.submenu, !sameShape(children, sub.items) { return false }
        }
        return true
    }

    private func makeItem(_ e: MenuEntry) -> NSMenuItem {
        if e.separator { return .separator() }
        let it = NSMenuItem()
        configure(it, e)
        if let children = e.children {
            let sub = NSMenu(title: e.title)
            sub.autoenablesItems = false
            for c in children { sub.addItem(makeItem(c)) }
            it.submenu = sub
        }
        return it
    }

    private func configure(_ it: NSMenuItem, _ e: MenuEntry) {
        if it.title != e.title { it.title = e.title }
        if it.isEnabled != e.enabled { it.isEnabled = e.enabled }
        let state: NSControl.StateValue = e.checked ? .on : .off
        if it.state != state { it.state = state }
        if it.keyEquivalent != e.key { it.keyEquivalent = e.key }
        let image = e.dot?.image
        if it.image !== image { it.image = image }
        // macOS 27 cache d'ordinaire les images des menus : le point d'état reste visible.
        if #available(macOS 27.0, *) { it.preferredImageVisibility = image == nil ? .automatic : .visible }
        if let action = e.action {
            it.target = self
            it.action = #selector(run(_:))
            if (it.representedObject as? ActionBox)?.action != action { it.representedObject = ActionBox(action) }
        } else {
            it.target = nil
            it.action = nil
            it.representedObject = nil
        }
    }

    @objc private func run(_ sender: NSMenuItem) {
        guard let action = (sender.representedObject as? ActionBox)?.action else { return }
        app.perform(action)
    }
}

/// Le symbole de KyberNow Link pour la barre des menus, une seule couleur
/// (image modèle) : la moitié « vous » pleine, la moitié « ordinateur » en
/// contour ; en session, les deux moitiés pleines (le lien établi). Carré de
/// 14 points sur 18, posé sur des points entiers : net à 16 et 32 pixels.
enum MenuBarIcon {
    static func image(lit: Bool) -> NSImage {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: true) { _ in
            guard let ctx = NSGraphicsContext.current?.cgContext else { return false }
            let mark = CGRect(x: 2, y: 2, width: 14, height: 14)
            let source = MarkGeometry.source(in: mark).cgPath
            let dest = MarkGeometry.dest(in: mark).cgPath
            ctx.setFillColor(NSColor.black.cgColor)
            ctx.setStrokeColor(NSColor.black.cgColor)
            ctx.addPath(dest)
            ctx.fillPath()
            if lit {
                ctx.addPath(source)
                ctx.fillPath()
            } else {
                // Contour d'un point, à l'intérieur de la forme.
                ctx.saveGState()
                ctx.addPath(source)
                ctx.clip()
                ctx.addPath(source)
                ctx.setLineWidth(2)
                ctx.strokePath()
                ctx.restoreGState()
            }
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = "KyberNow Link"
        return image
    }
}
