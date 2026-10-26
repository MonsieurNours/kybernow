// Notifications du système (docs/link-ui.md, « Notifications »), seulement
// quand l'app n'est pas au premier plan : mise à jour de la Station prête (une
// fois par version et par ordinateur), ordinateur en ligne après un réveil, ou
// toujours muet au bout de 2 min, session arrêtée sans « Se déconnecter », fin
// d'un redémarrage ou d'une mise à jour lancés hors de la fenêtre ; et, une
// seule fois, « KyberNow Link est toujours là » à la première fermeture de la
// fenêtre. L'autorisation est demandée à la première notification. Un clic
// ouvre l'app sur ce qu'elle annonce ; ses boutons agissent comme le menu.

import AppKit
import Combine
import KyberKit
import UserNotifications

@MainActor
final class Notifier: NSObject {
    enum Kind: String {
        case stationUpdate = "station-update"
        case stationSetup = "station-setup"
        case online
        case wakeTimeout = "wake-timeout"
        case sessionEnded = "session-ended"
        case sessionStopped = "session-stopped"
        case stationDone = "station-done"
        case stillHere = "still-here"
    }

    /// Boutons des notifications.
    enum Button: String {
        case connect, update
        case takeBack = "take-back"
        case resume
    }

    private weak var app: AppModel?
    private var watchers = Set<AnyCancellable>()
    /// Réveils en cours, pour savoir lesquels viennent de finir.
    private var waking: Set<UUID> = []
    /// Redémarrages et mises à jour lancés hors de la fenêtre (menu,
    /// notification) : leur fin est notifiée.
    private(set) var following: Set<UUID> = []

    /// Versions de la Station déjà annoncées, par ordinateur.
    private static let announcedKey = "stationUpdatesAnnounced"
    /// « KyberNow Link est toujours là » a été dit.
    private static let stillHereKey = "stillHereShown"

    private var center: UNUserNotificationCenter { .current() }

    func install(_ app: AppModel) {
        self.app = app
        center.delegate = self
        center.setNotificationCategories([
            category(.stationUpdate, .update, String(localized: "Mettre à jour"), foreground: false),
            category(.online, .connect, String(localized: "Se connecter")),
            category(.sessionEnded, .takeBack, String(localized: "Reprendre la main")),
            category(.sessionStopped, .resume, String(localized: "Reprendre")),
        ])
        let store = app.store
        store.$status.receive(on: DispatchQueue.main)
            .sink { [weak self] _ in self?.checkStationUpdates() }.store(in: &watchers)
        store.$waking.receive(on: DispatchQueue.main)
            .sink { [weak self] in self?.wakingChanged($0) }.store(in: &watchers)
        store.$stationTasks.receive(on: DispatchQueue.main)
            .sink { [weak self] in self?.tasksChanged($0) }.store(in: &watchers)
        // Une mise à jour vue pendant que l'app était au premier plan s'annonce dès qu'elle ne l'est plus.
        NotificationCenter.default.publisher(for: NSApplication.didResignActiveNotification)
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in self?.checkStationUpdates() }.store(in: &watchers)
    }

    private func category(_ kind: Kind, _ button: Button, _ title: String, foreground: Bool = true) -> UNNotificationCategory {
        let action = UNNotificationAction(identifier: button.rawValue, title: title, options: foreground ? [.foreground] : [])
        return UNNotificationCategory(identifier: kind.rawValue, actions: [action], intentIdentifiers: [])
    }

    // MARK: - Ce qui est annoncé

    /// Une version de la Station est prête : une fois par version et par ordinateur.
    func checkStationUpdates() {
        guard let app, !NSApp.isActive else { return }
        var announced = app.defaults.dictionary(forKey: Self.announcedKey) as? [String: String] ?? [:]
        for host in app.store.hosts {
            guard let station = app.store.status[host.id]?.info?.station, station.updateAvailable,
                  let v = station.updateVersion, station.installable || station.requiresSetup,
                  !station.installing, app.store.stationTasks[host.id]?.working != true,
                  announced[host.id.uuidString] != v else { continue }
            announced[host.id.uuidString] = v
            let title = French.elides(host.name)
                ? String(localized: "Mise à jour de la Station d'\(host.name)")
                : String(localized: "Mise à jour de la Station de \(host.name)")
            if station.installable {
                post(.stationUpdate, host: host.id, title: title,
                     body: String(localized: "La version \(v) est prête. Environ une minute ; une session en cours sera coupée."))
            } else {
                post(.stationSetup, host: host.id, title: title, body: StationText.requiresSetup(v))
            }
        }
        app.defaults.set(announced, forKey: Self.announcedKey)
    }

    /// Fin d'un réveil : en ligne, ou toujours rien au bout de 2 min.
    private func wakingChanged(_ now: Set<UUID>) {
        let finished = waking.subtracting(now)
        waking = now
        guard let app else { return }
        for id in finished {
            guard let host = app.store.host(id) else { continue }
            if app.store.status[id]?.online == true {
                post(.online, host: id, title: String(localized: "\(host.name) est en ligne"),
                     body: String(localized: "Vous pouvez vous connecter."))
            } else {
                post(.wakeTimeout, host: id, title: String(localized: "\(host.name) ne répond toujours pas"),
                     body: String(localized: "Vérifiez qu'il est branché au réseau et que le réveil est autorisé sur l'ordinateur (Admin › Système)."))
            }
        }
    }

    func follow(_ id: UUID) { following.insert(id) }
    func unfollow(_ id: UUID) { following.remove(id) }

    /// Fin d'un redémarrage ou d'une mise à jour lancés hors de la fenêtre :
    /// les textes de fin de la rubrique Station (ou son refus : trop tôt…).
    private func tasksChanged(_ tasks: [UUID: StationTask]) {
        for id in following {
            switch tasks[id] {
            case let .done(text)?, let .failed(text)?:
                following.remove(id)
                if let host = app?.store.host(id) { post(.stationDone, host: id, title: host.name, body: text) }
            default:
                break
            }
        }
    }

    /// Une session s'est arrêtée sans « Se déconnecter » : un autre appareil
    /// ou Admin y a mis fin, ou une erreur (sa raison, en une phrase).
    func sessionEnded(_ host: Host, error: String?) {
        if let error {
            post(.sessionStopped, host: host.id, title: String(localized: "La session sur \(host.name) s'est arrêtée"), body: Self.sentence(error))
        } else {
            post(.sessionEnded, host: host.id, title: String(localized: "La session sur \(host.name) est terminée"),
                 body: String(localized: "Un autre appareil a pris la main, ou Admin y a mis fin."))
        }
    }

    /// Première fermeture de la fenêtre, une seule fois.
    func stillHere() {
        guard let app, !app.defaults.bool(forKey: Self.stillHereKey) else { return }
        app.defaults.set(true, forKey: Self.stillHereKey)
        post(.stillHere, host: nil, title: String(localized: "KyberNow Link est toujours là"),
             body: String(localized: "Vos ordinateurs vous attendent dans la barre des menus."), always: true)
    }

    /// Les raisons du cœur commencent par une minuscule, sans point.
    static func sentence(_ text: String) -> String {
        var s = text.prefix(1).uppercased() + text.dropFirst()
        if let last = s.last, !".!?…".contains(last) { s += "." }
        return s
    }

    // MARK: - Envoi

    /// Seulement quand l'app n'est pas au premier plan (`always` : la
    /// première fermeture de la fenêtre). Une notification remplace la
    /// précédente du même genre pour le même ordinateur.
    private func post(_ kind: Kind, host: UUID?, title: String, body: String, always: Bool = false) {
        guard always || !NSApp.isActive else { return }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.categoryIdentifier = kind.rawValue
        content.userInfo = ["kind": kind.rawValue, "host": host?.uuidString ?? ""]
        let id = host.map { "\(kind.rawValue)-\($0.uuidString)" } ?? kind.rawValue
        KyberLog.app.notice("notification \(kind.rawValue, privacy: .public) : \(title, privacy: .public) — \(body, privacy: .public)")
        #if DEBUG
        DemoRemote.journal("notification \(kind.rawValue) | \(title) | \(body)")
        // Démonstration : rien vers le centre de notifications sans `-demoNotify YES`
        // (même identifiant que l'app installée : à faire avec une copie réidentifiée).
        if AppModel.demo, !UserDefaults.standard.bool(forKey: "demoNotify") { return }
        #endif
        let request = UNNotificationRequest(identifier: id, content: content, trigger: nil)
        Task {
            guard await self.allowed() else { return }
            do {
                try await self.center.add(request)
            } catch {
                KyberLog.app.error("notification refusée : \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    /// Autorisation demandée la première fois qu'il y a quelque chose à dire.
    private func allowed() async -> Bool {
        switch await center.notificationSettings().authorizationStatus {
        case .notDetermined:
            var options: UNAuthorizationOptions = [.alert, .sound]
            #if DEBUG
            // Démonstration : livrées sans demande, dans le centre de notifications.
            if AppModel.demo { options = [.alert, .provisional] }
            #endif
            return (try? await center.requestAuthorization(options: options)) ?? false
        case .denied:
            return false
        default:
            return true
        }
    }

    // MARK: - Réponses

    /// Un clic sur la notification ouvre l'app sur ce qu'elle annonce (l'accueil
    /// sur l'ordinateur, ou sa rubrique Station) ; un bouton agit.
    func respond(kind: String, host: String, action: String) {
        guard let app else { return }
        let h = UUID(uuidString: host).flatMap { app.store.host($0) }
        switch action {
        case Button.connect.rawValue, Button.takeBack.rawValue, Button.resume.rawValue:
            if let h { app.connectFromOutside(h) }
        case Button.update.rawValue:
            if let h { app.stationFromOutside(.update, h) }
        case UNNotificationDefaultActionIdentifier:
            switch Kind(rawValue: kind) {
            case .stationUpdate?, .stationSetup?, .stationDone?:
                if let h { app.openSettingsFromOutside(h, section: .station) } else { app.showWindow() }
            case .online?, .wakeTimeout?, .sessionEnded?, .sessionStopped?:
                app.openHome(selecting: h)
            default:
                app.showWindow()
            }
        default:
            break
        }
    }
}

extension Notifier: UNUserNotificationCenterDelegate {
    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification,
                                            withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        let kind = notification.request.content.userInfo["kind"] as? String
        DispatchQueue.main.async {
            // Au premier plan, l'app le montre déjà ; « toujours là » se dit en fermant la fenêtre.
            let show = kind == Kind.stillHere.rawValue || !NSApp.isActive
            completionHandler(show ? [.banner, .list] : [])
        }
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse,
                                            withCompletionHandler completionHandler: @escaping () -> Void) {
        let info = response.notification.request.content.userInfo
        let kind = info["kind"] as? String ?? ""
        let host = info["host"] as? String ?? ""
        let action = response.actionIdentifier
        DispatchQueue.main.async {
            MainActor.assumeIsolated { self.respond(kind: kind, host: host, action: action) }
            completionHandler()
        }
    }
}
