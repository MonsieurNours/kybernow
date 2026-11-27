// Essais de SessionRules.swift (apps/ios/KyberNowIOS), lancés par run.sh.

import Foundation

var failures = 0
func check(_ ok: Bool, _ what: String, line: Int = #line) {
    if !ok {
        failures += 1
        print("ÉCHEC (ligne \(line)) : \(what)")
    }
}

let alive: [SessionStep] = [.preparing, .connecting, .starting, .streaming, .reconnecting]
let over: [SessionStep] = [.ending, .ended, .failed]

// Veille : retenue de la préparation à la fin, reconnexion comprise (IOS-04).
for s in alive { check(SessionRules.keepsAwake(s), "veille retenue en \(s)") }
for s in over { check(!SessionRules.keepsAwake(s), "veille rendue en \(s)") }

// Premier plan perdu : tout est relâché à toute étape vivante (IOS-16).
for s in alive { check(SessionRules.reactsToBackground(s), "arrière-plan traité en \(s)") }
for s in over { check(!SessionRules.reactsToBackground(s), "arrière-plan ignoré en \(s)") }

// Apple TV : le menu s'ouvre pendant l'image et pendant la reconnexion (TVOS-02).
check(SessionRules.opensMenu(.streaming), "menu pendant l'image")
check(SessionRules.opensMenu(.reconnecting), "menu pendant la reconnexion")
for s: SessionStep in [.preparing, .connecting, .starting, .ending, .ended, .failed] {
    check(!SessionRules.opensMenu(s), "pas de menu en \(s)")
}
// … et Retour revient à l'app pendant la reconnexion (la télécommande n'est au jeu que pendant l'image).
check(SessionRules.gameOwnsRemote(.streaming), "télécommande au jeu pendant l'image")
check(!SessionRules.gameOwnsRemote(.reconnecting), "Retour à l'app pendant la reconnexion")

// Plusieurs sessions retiennent la veille : elle revient quand la dernière la rend.
final class Owner {}
let a = Owner(), b = Owner()
var holds = WakeHolds()
check(!holds.held, "rien de retenu au départ")
check(holds.set(ObjectIdentifier(a), true), "a retient")
check(holds.set(ObjectIdentifier(b), true), "b retient")
check(holds.set(ObjectIdentifier(a), false), "b retient encore quand a rend")
check(holds.set(ObjectIdentifier(a), false), "rendre deux fois ne change rien")
check(!holds.set(ObjectIdentifier(b), false), "plus rien quand b rend")
check(!holds.held, "veille rendue")

if failures == 0 {
    print("session-rules : tout est bon")
} else {
    print("session-rules : \(failures) échec(s)")
    exit(1)
}
