// Essai des règles sans état de l'app (KyberNow/SessionRules.swift).
import Foundation

var failures = 0
func expect(_ ok: Bool, _ what: String, line: Int = #line) {
    if !ok { failures += 1; print("ÉCHEC (ligne \(line)) : \(what)") }
}

// MAC-07 : après la fin, seule la fin du cœur passe.
expect(!SessionRules.ignoresAfterEnd(ending: false, incomingIsEnded: false), "en session, un état passe")
expect(SessionRules.ignoresAfterEnd(ending: true, incomingIsEnded: false), "après end(), un état de connexion est ignoré")
expect(!SessionRules.ignoresAfterEnd(ending: true, incomingIsEnded: true), "après end(), la fin du cœur passe")

// MAC-06 : modificateurs d'après leurs bits.
let leftCmd: UInt64 = 0x08, rightCmd: UInt64 = 0x10, command: UInt64 = 0x10_0000
expect(SessionRules.modifierPressed(code: 0x37, flags: command | leftCmd) == true, "⌘ gauche appuyé")
expect(SessionRules.modifierPressed(code: 0x37, flags: 0) == false, "⌘ gauche relâché")
// ⌘ droit encore tenu : le relâchement du gauche reste un relâchement.
expect(SessionRules.modifierPressed(code: 0x37, flags: command | rightCmd) == false, "⌘ gauche relâché, droit tenu")
expect(SessionRules.modifierPressed(code: 0x3E, flags: 0x2000) == true, "⌃ droit appuyé")
expect(SessionRules.modifierPressed(code: 0x39, flags: 0x1_0000) == nil, "Verr. Maj bascule")
expect(SessionRules.modifierPressed(code: 0x00, flags: 0) == nil, "une lettre n'est pas un modificateur")

if failures > 0 { print("\(failures) échec(s)"); exit(1) }
print("règles : tout passe")
