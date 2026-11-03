// Textes de l'icône, en français et en anglais (docs/i18n.md) : français si
// la première langue préférée de macOS l'est, anglais sinon. KYBERNOW_LANG
// force la langue pour les essais.

import Foundation

enum Lang { case fr, en }

let lang: Lang = {
    if let forced = ProcessInfo.processInfo.environment["KYBERNOW_LANG"], !forced.isEmpty {
        return forced.lowercased().hasPrefix("fr") ? .fr : .en
    }
    return (Locale.preferredLanguages.first ?? "fr").lowercased().hasPrefix("fr") ? .fr : .en
}()

private let fr: [String: String] = [
    "status.running": "Station en marche",
    "status.starting": "Démarrage de la Station…",
    "status.stopped": "Station arrêtée",
    "status.adminDown": "Admin ne répond pas",
    "status.permissions": "Il manque des autorisations",
    "session.connected": "{name} est connecté",
    "session.none": "Aucune session en cours",
    "session.last": "Dernière session : {name}, {when}",
    "session.mode": "{w} × {h} · {hz} Hz",
    "session.sharing": "Partage de l'écran en cours",
    "session.lent": "Ordinateur prêté",
    "lend.lentTo": "Prêté à {name}",
    "lend.lentToUntil": "Prêté à {name} jusqu'à {time}",
    "lend.takingBackIn": "Reprise dans {left}",
    "lend.wants": "{name} veut emprunter l'ordinateur",
    "lend.accept": "Accepter",
    "lend.refuse": "Refuser",
    "menu.takeBack": "Reprendre l'ordinateur",
    "menu.takeBackNow": "Reprendre tout de suite",
    "session.requests": "{n} demande(s) d'accès",
    "ago.minutes": "il y a {n} min",
    "ago.today": "aujourd'hui",
    "ago.yesterday": "hier",
    "ago.days": "il y a {n} jours",
    "menu.end": "Mettre fin à la session",
    "menu.requests": "Vérifier la demande dans Admin…",
    "menu.open": "Ouvrir Admin",
    "menu.share": "Partager un écran…",
    "menu.lend": "Prêter l'ordinateur…",
    "menu.update": "Mettre à jour ({version})…",
    "menu.updates": "Mises à jour…",
    "menu.restart": "Redémarrer la Station",
    "menu.stop": "Arrêter la Station",
    "menu.start": "Démarrer la Station",
    "menu.startWithMac": "Démarrer avec le Mac",
    "menu.permissions": "Autorisations…",
    "menu.permissionsCount": "{n} sur 3",
    "menu.reset": "Réinitialiser le mot de passe d'Admin…",
    "menu.uninstall": "Désinstaller KyberNow Station…",
    "menu.quit": "Fermer l'icône",
    "reset.title": "Réinitialiser le mot de passe d'Admin ?",
    "reset.text": "Le mot de passe d'Admin est effacé, et Admin vous donne un code pour en choisir un nouveau. Vos appareils et leurs comptes ne changent pas.",
    "reset.ok": "Réinitialiser",
    "reset.cancel": "Annuler",
    "reset.code": "Code d'initialisation : {code}",
    "reset.codeText": "Ouvrez Admin et saisissez ce code pour choisir un nouveau mot de passe.",
    "reset.openAdmin": "Ouvrir Admin",
    "reset.copy": "Copier le code",
    "reset.failed": "Le mot de passe n'a pas été réinitialisé.",
    "quit.title": "Fermer l'icône de KyberNow Station ?",
    "quit.text": "La Station peut rester en marche : vos appareils pourront toujours se connecter, et vous rouvrirez l'icône en ouvrant KyberNow Station dans Applications.",
    "quit.keep": "Laisser la Station en marche",
    "quit.stop": "Arrêter aussi la Station",
    "quit.cancel": "Annuler",
    "error.title": "KyberNow Station",
]

private let en: [String: String] = [
    "status.running": "Station running",
    "status.starting": "Starting the Station…",
    "status.stopped": "Station stopped",
    "status.adminDown": "Admin isn't answering",
    "status.permissions": "Some permissions are missing",
    "session.connected": "{name} is connected",
    "session.none": "No session running",
    "session.last": "Last session: {name}, {when}",
    "session.mode": "{w} × {h} · {hz} Hz",
    "session.sharing": "Screen sharing in progress",
    "session.lent": "Computer on loan",
    "lend.lentTo": "Lent to {name}",
    "lend.lentToUntil": "Lent to {name} until {time}",
    "lend.takingBackIn": "Taking back in {left}",
    "lend.wants": "{name} wants to borrow the computer",
    "lend.accept": "Accept",
    "lend.refuse": "Refuse",
    "menu.takeBack": "Take the computer back",
    "menu.takeBackNow": "Take back right away",
    "session.requests": "{n} access request(s)",
    "ago.minutes": "{n} min ago",
    "ago.today": "today",
    "ago.yesterday": "yesterday",
    "ago.days": "{n} days ago",
    "menu.end": "End the session",
    "menu.requests": "Check the request in Admin…",
    "menu.open": "Open Admin",
    "menu.share": "Share a screen…",
    "menu.lend": "Lend the computer…",
    "menu.update": "Update ({version})…",
    "menu.updates": "Updates…",
    "menu.restart": "Restart the Station",
    "menu.stop": "Stop the Station",
    "menu.start": "Start the Station",
    "menu.startWithMac": "Start with the Mac",
    "menu.permissions": "Permissions…",
    "menu.permissionsCount": "{n} of 3",
    "menu.reset": "Reset the Admin password…",
    "menu.uninstall": "Uninstall KyberNow Station…",
    "menu.quit": "Close the icon",
    "reset.title": "Reset the Admin password?",
    "reset.text": "The Admin password is erased, and Admin gives you a code to choose a new one. Your devices and their accounts don't change.",
    "reset.ok": "Reset",
    "reset.cancel": "Cancel",
    "reset.code": "Setup code: {code}",
    "reset.codeText": "Open Admin and enter this code to choose a new password.",
    "reset.openAdmin": "Open Admin",
    "reset.copy": "Copy the code",
    "reset.failed": "The password wasn't reset.",
    "quit.title": "Close the KyberNow Station icon?",
    "quit.text": "The Station can keep running: your devices can still connect, and you can reopen the icon by opening KyberNow Station in Applications.",
    "quit.keep": "Keep the Station running",
    "quit.stop": "Also stop the Station",
    "quit.cancel": "Cancel",
    "error.title": "KyberNow Station",
]

/// Texte de `key`, `{nom}` remplacé par sa valeur.
func t(_ key: String, _ vars: [String: CustomStringConvertible] = [:]) -> String {
    var text = (lang == .fr ? fr[key] : en[key]) ?? fr[key] ?? key
    for (name, value) in vars {
        text = text.replacingOccurrences(of: "{\(name)}", with: value.description)
    }
    return text
}

/// Les deux catalogues ont les mêmes clés et les mêmes {variables} (vérifié par `--check-strings`).
func checkStrings() -> Bool {
    let vars = { (s: String) -> [String] in
        let regex = try! NSRegularExpression(pattern: "\\{(\\w+)\\}")
        return regex.matches(in: s, range: NSRange(s.startIndex..., in: s)).map { String(s[Range($0.range(at: 1), in: s)!]) }.sorted()
    }
    var ok = Set(fr.keys) == Set(en.keys)
    for (k, v) in fr where vars(v) != vars(en[k] ?? "") {
        print("variables différentes : \(k)")
        ok = false
    }
    if !ok { print("catalogues fr/en différents") }
    return ok
}
