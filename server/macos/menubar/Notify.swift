// Les demandes qui attendent la personne devant le Mac, en notifications de
// macOS : sous Windows, le panneau de l'icône s'ouvre de lui-même (server/tray,
// `app.rs`) ; ici, une notification par demande nouvelle, une fois chacune.
//
// - emprunt de l'ordinateur (docs/pret.md) : « Léa veut emprunter
//   l'ordinateur », avec « Accepter » et « Refuser » (le compte Steam choisi
//   avec le code ou sur le compte de l'ami) ; cliquée, la page « Prêt » d'Admin ;
// - accès d'un appareil : « iPad demande l'accès » ; cliquée, la page des appareils.
//
// Une demande traitée ou expirée retire sa notification.

import Foundation
import UserNotifications

final class Notifier: NSObject, UNUserNotificationCenterDelegate {
    private let center = UNUserNotificationCenter.current()
    private var lend: Set<String> = []
    private var pair: Set<String> = []
    /// « Accepter » ou « Refuser » : identifiant de la demande, accord, compte Steam.
    var onAnswer: ((String, Bool, String) -> Void)?
    /// Une notification cliquée : la page d'Admin.
    var onOpen: ((String) -> Void)?

    override init() {
        super.init()
        center.delegate = self
        let accept = UNNotificationAction(identifier: "accept", title: t("lend.accept"), options: [])
        let refuse = UNNotificationAction(identifier: "refuse", title: t("lend.refuse"), options: [.destructive])
        center.setNotificationCategories([
            UNNotificationCategory(identifier: "lend", actions: [accept, refuse], intentIdentifiers: [], options: []),
            UNNotificationCategory(identifier: "pair", actions: [], intentIdentifiers: [], options: []),
        ])
        center.requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    /// À chaque relevé : une notification par demande nouvelle, retirée quand elle n'est plus là.
    func update(_ s: Snapshot) {
        lend = sync(s.lendRequests, known: lend, prefix: "lend") { request in
            let content = UNMutableNotificationContent()
            content.title = t("lend.wants", ["name": request["name"] as? String ?? ""])
            content.body = t((request["steam"] as? String) == "host" ? "notify.steamHost" : "notify.steamFriend")
            content.categoryIdentifier = "lend"
            content.userInfo = ["id": request["id"] as? String ?? "", "steam": request["steam"] as? String ?? "friend"]
            return content
        }
        pair = sync(s.pairRequests, known: pair, prefix: "pair") { request in
            let content = UNMutableNotificationContent()
            content.title = t("notify.pair", ["name": request["name"] as? String ?? ""])
            content.body = t("notify.pairText")
            content.categoryIdentifier = "pair"
            return content
        }
    }

    private func sync(_ requests: [[String: Any]], known: Set<String>, prefix: String, content: ([String: Any]) -> UNMutableNotificationContent) -> Set<String> {
        var now: Set<String> = []
        for request in requests {
            guard let id = request["id"] as? String else { continue }
            now.insert(id)
            if known.contains(id) { continue }
            let note = content(request)
            note.sound = .default
            center.add(UNNotificationRequest(identifier: "\(prefix)-\(id)", content: note, trigger: nil))
        }
        let gone = known.subtracting(now).map { "\(prefix)-\($0)" }
        if !gone.isEmpty {
            center.removeDeliveredNotifications(withIdentifiers: gone)
            center.removePendingNotificationRequests(withIdentifiers: gone)
        }
        return now
    }

    // L'icône n'a pas de fenêtre au premier plan : la notification s'affiche quand même.
    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification,
                                withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        completionHandler([.banner, .list, .sound])
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse,
                                withCompletionHandler completionHandler: @escaping () -> Void) {
        let content = response.notification.request.content
        let info = content.userInfo
        let id = info["id"] as? String ?? ""
        let steam = info["steam"] as? String ?? "friend"
        let lendPage = content.categoryIdentifier == "lend"
        DispatchQueue.main.async { [weak self] in
            switch response.actionIdentifier {
            case "accept": self?.onAnswer?(id, true, steam)
            case "refuse": self?.onAnswer?(id, false, steam)
            default: self?.onOpen?(lendPage ? "pret" : "appareils")
            }
        }
        completionHandler()
    }
}
