// Test hors ligne du module d'entrées partagé de KyberKit, sans ordinateur ni
// matériel : Keymap (quelques touches, clavier ISO ou non, Verr. maj.),
// PadCombo (sous verrou comme sur Mac ; combinaison brève, tenue, relâchement
// complet comme sur Apple TV), la file de secours
// de PacketQueue (envoi à kynput remplacé par un envoi qui refuse à volonté)
// et la règle de filtrage des axes d'InputPipeline.

import CKyber
import Foundation

var failed = false
func check(_ ok: Bool, _ what: String) {
    print(ok ? "ok   " : "ÉCHEC", what)
    if !ok { failed = true }
}
func sleep(ms: Int) { Thread.sleep(forTimeInterval: Double(ms) / 1000) }

// MARK: - Keymap

print("— Keymap")
func kyber(_ usage: UInt16) -> UInt16? {
    var scancode: UInt16 = 0
    return kynput_keycode_usb_to_kyber(usage, &scancode) ? scancode : nil
}
check(Keymap.scancode(hid: 0x04, iso: false) == 0x1E, "A (usage 0x04) → scancode 0x1E")
check(Keymap.scancode(hid: 0x28, iso: false) == 0x1C, "Entrée (0x28) → 0x1C")
check(Keymap.scancode(hid: 0x29, iso: false) == 0x01, "Échap (0x29) → 0x01")
check(Keymap.scancode(hid: 0x50, iso: false) == kyber(0x50), "flèche gauche : même scancode que kynput")
check(Keymap.scancode(hid: 0x1234, iso: false) == nil, "usage inconnu : nil")
check(Keymap.usage(hid: 0x35, iso: false) == 0x35 && Keymap.usage(hid: 0x64, iso: false) == 0x64, "clavier ANSI : `<>` et la touche sous Échap inchangées")
check(Keymap.usage(hid: 0x35, iso: true) == 0x64 && Keymap.usage(hid: 0x64, iso: true) == 0x35, "clavier Apple ISO : 0x35 ↔ 0x64 échangés")
check(Keymap.usage(hid: 0x04, iso: true) == 0x04, "clavier Apple ISO : les autres touches inchangées")
check(Keymap.scancode(hid: 0x64, iso: true) == kyber(0x35), "clavier Apple ISO : la touche sous Échap part avec le scancode de `<>`")
var evaluated = false
_ = Keymap.usage(hid: 0x04, iso: { evaluated = true; return true }())
check(!evaluated, "la disposition n'est interrogée que pour les deux touches échangées")
check(Keymap.usage(hid: 0xE3, commandAsControl: true, iso: false) == 0xE0 && Keymap.usage(hid: 0xE7, commandAsControl: true, iso: false) == 0xE4, "⌘ envoyée comme Ctrl, des deux côtés")
check(Keymap.usage(hid: 0xE3, commandAsControl: false, iso: false) == 0xE3, "⌘ gardée sinon")
check(Keymap.isLock(Keymap.capsLock) && Keymap.isLock(Keymap.numLock) && !Keymap.isLock(0x04) && !Keymap.isLock(0xE1), "Verr. maj. et Verr. num. sont des bascules, pas A ni ⇧")
check(Keymap.scancode(hid: Keymap.capsLock, iso: true) == 0x3A, "Verr. maj. → scancode 0x3A, même sur un clavier ISO")
check(Keymap.isModifier(0xE0) && Keymap.isModifier(0xE7) && !Keymap.isModifier(0xDF) && !Keymap.isModifier(Keymap.capsLock), "modificateurs : usages 0xE0 à 0xE7 seulement")
check(Keymap.appleKeyboard(vendorName: nil) && Keymap.appleKeyboard(vendorName: "Apple Inc.") && !Keymap.appleKeyboard(vendorName: "Logitech"), "réglage ISO : claviers Apple ou inconnus")

// MARK: - PadCombo

print("— PadCombo, sous un verrou (Mac : hook de kynput, minuterie sur une autre file)")
let lock = NSLock()
var formed: [UInt32] = []
let gate = PadCombo(on: DispatchQueue(label: "test.minuterie"))
gate.exclusive = { work in lock.lock(); work(); lock.unlock() }
gate.onFormed = { id in
    // Appelé dans `exclusive` : le verrou est tenu (un second lock() bloquerait).
    formed.append(id)
}
let pad: UInt32 = 1
func hook(_ button: PadButton, _ down: Bool) { lock.lock(); gate.button(pad, button, pressed: down); lock.unlock() }
hook(.leftShoulder, true); hook(.rightShoulder, true); hook(.start, true); hook(.back, true)
check(lock.withLock { gate.pressed(pad) } == [.leftShoulder, .rightShoulder, .start, .back] && lock.withLock { gate.pads } == [pad], "touches tenues et manette connues")
sleep(ms: 350)
check(formed == [pad] && lock.withLock { gate.formed(pad) }, "formée une fois après 250 ms, la minuterie a pris le verrou")
for b in PadCombo.buttons { hook(b, false) }
hook(.a, true); hook(.a, false)
check(lock.withLock { gate.pressed(pad).isEmpty && !gate.formed(pad) }, "tout relâché : plus rien de formé ni de tenu")
lock.withLock { gate.enabled = false }
for b in PadCombo.buttons { hook(b, true) }
sleep(ms: 350)
check(formed.count == 1, "désactivée : rien ne se forme")
lock.withLock { gate.forget(pad) }
check(lock.withLock { gate.pads.isEmpty && gate.pressed(pad).isEmpty }, "manette oubliée")

print("— PadCombo, formée après 250 ms (Apple TV)")
let queue = DispatchQueue(label: "test.entrees")
var held: [UInt32] = []
let combo = PadCombo(on: queue)
combo.onFormed = { held.append($0) }
func press(_ buttons: [PadButton], _ down: Bool) {
    queue.sync { for b in buttons { _ = combo.button(2, b, pressed: down) } }
}
// Brève : une touche lâchée avant 250 ms.
press([.leftShoulder, .rightShoulder, .start, .back], true)
sleep(ms: 100)
press([.start], false)
sleep(ms: 350)
check(held.isEmpty && queue.sync { !combo.formed(2) }, "combinaison brève : rien ne se forme")
press([.leftShoulder, .rightShoulder, .back], false)
// Tenue.
press([.leftShoulder, .rightShoulder, .start, .back], true)
sleep(ms: 120)
check(held.isEmpty, "pas encore formée à 120 ms")
sleep(ms: 250)
check(held == [2] && queue.sync { combo.formed(2) }, "formée une fois après 250 ms tenues")
sleep(ms: 300)
check(held.count == 1, "pas reformée tant que rien ne bouge")
// Relâchement complet.
press([.leftShoulder, .rightShoulder, .start], false)
check(queue.sync { combo.formed(2) && combo.pressed(2) == [.back] }, "formée tant que Select est tenu")
press([.back], false)
check(queue.sync { combo.formed(2) }, "formée encore au dernier relâchement")
press([.a], true)
press([.a], false)
check(queue.sync { !combo.formed(2) && combo.pressed(2).isEmpty }, "tout relâché : plus rien de formé ni de tenu")
// Resynchronisation (menu fermé) et réglage.
queue.sync { combo.set(pressed: [.leftShoulder, .y], for: 2) }
check(queue.sync { combo.pressed(2) } == [.leftShoulder, .y], "état réel repris")
queue.sync { combo.enabled = false }
press([.rightShoulder, .start, .back], true)
sleep(ms: 350)
check(held.count == 1, "réglage coupé : rien ne s'arme")
press([.leftShoulder, .rightShoulder, .start, .back, .y], false)
check(PadCombo.navigationKey(.dpadLeft) == .left && PadCombo.navigationKey(.a) == .accept && PadCombo.navigationKey(.b) == .back && PadCombo.navigationKey(.x) == nil,
      "touches de navigation : croix, A, B")
check(PadButton(sdk: KYNPUT_GAMEPAD_BUTTON_D_PAD_UP) == .dpadUp && PadButton.allCases.allSatisfy { PadButton(sdk: $0.sdk) == $0 }, "touches kynput ↔ PadButton")

// MARK: - PacketQueue

print("— PacketQueue, file de secours")
final class Host {
    var refuse = 0            // refus restants avant d'accepter
    var refuseAll = false
    var received: [String] = []
    var consumed = 0
    func consume(_ packet: OpaquePointer) -> Int32 {
        consumed += 1
        defer { kynput_packet_unref(packet) }
        if refuseAll || refuse > 0 {
            refuse = max(0, refuse - 1)
            return -1
        }
        if let key = kynput_keyboard_key_from_packet(packet) {
            defer { kynput_keyboard_key_unref(key) }
            received.append("touche")
        } else if let pos = kynput_mouse_position_from_packet(packet) {
            defer { kynput_mouse_position_unref(pos) }
            received.append("position \(kynput_mouse_position_x(pos))")
        } else {
            received.append("?")
        }
        return 0
    }
}
let host = Host()
let sendQueue = DispatchQueue(label: "test.envoi")
let packets = PacketQueue(queue: sendQueue) { host.consume($0) }
var finals: [String] = []
var firsts: [String] = []
packets.onDropped = { kind, final, _ in if final { finals.append(kind) } }
packets.onFirst = { kind, _ in firsts.append(kind) }
func key(_ scancode: UInt16, _ pressed: Bool) -> (() -> OpaquePointer?) {
    { kynput_keyboard_key_to_packet(KYNPUT_INPUT_TARGET_HOST, scancode, pressed) }
}
func position(_ x: Int16) -> OpaquePointer? { kynput_mouse_position_to_packet(KYNPUT_INPUT_TARGET_HOST, 1, x, 0) }

// Tout accepté : rien n'attend.
packets.sendReliably("touche", key(0x1E, true))
check(host.received == ["touche"] && !packets.hasBacklog && packets.droppedPackets == 0, "paquet accepté : parti tout de suite")
check(packets.send(position(10), "position") && host.received.last == "position 10", "position acceptée")
check(firsts == ["touche", "position"], "premier envoi de chaque sorte journalisé une fois")

// Refusé deux fois : réessayé toutes les 3 ms, puis parti ; ce qui suit garde l'ordre.
host.received = []
host.refuse = 2
var made = 0
packets.sendReliably("touche") { made += 1; return kynput_keyboard_key_to_packet(KYNPUT_INPUT_TARGET_HOST, 0x1E, false) }
check(host.received.isEmpty && packets.hasBacklog, "refusé : en file de secours")
packets.sendReliably("clic", key(0x2C, true))
check(host.consumed == 3, "derrière la file de secours, le suivant n'est pas tenté : il attend son tour")
check(!packets.send(position(20), "position") && packets.droppedPackets == 1, "une position derrière la file de secours est perdue, pas envoyée")
sleep(ms: 40)
check(host.received == ["touche", "touche"] && !packets.hasBacklog, "les deux paquets sont partis dans l'ordre après les réessais")
check(made == 3, "le paquet refusé est refait à chaque essai (kynput garde le paquet)")
check(packets.send(position(30), "position") && host.received.last == "position 30", "file vide : les positions repartent")

// Refusé sans fin : abandonné après 200 ms, compté.
host.received = []
host.refuseAll = true
packets.sendReliably("touche", key(0x10, true))
sleep(ms: 120)
check(packets.hasBacklog && finals.isEmpty, "toujours réessayé à 120 ms")
sleep(ms: 150)
check(!packets.hasBacklog && finals == ["touche"] && packets.droppedPackets == 2, "perdu après 200 ms de réessais, journalisé comme tel")
host.refuseAll = false

// Arrêt : plus rien ne part.
packets.sendReliably("touche", key(0x11, true))
host.received = []
let dropped = packets.stop()
check(dropped == 2, "l'arrêt rend le total des paquets perdus")
packets.sendReliably("touche", key(0x12, true))
check(!packets.send(position(40), "position") && host.received.isEmpty && !packets.hasBacklog, "arrêtée : rien n'est envoyé ni gardé")

// MARK: - Axes

print("— InputPipeline, filtrage des axes")
check(InputPipeline.axisMoved(from: nil, to: 0) && InputPipeline.axisMoved(from: nil, to: 5), "première valeur : passe toujours")
check(!InputPipeline.axisMoved(from: 1000, to: 1020) && InputPipeline.axisMoved(from: 1000, to: 1200), "changement dans la même tranche de 1/512 écarté, tranche changée gardée")
check(InputPipeline.axisMoved(from: 3, to: 0) && !InputPipeline.axisMoved(from: 0, to: 0), "retour exact au repos : passe toujours, sauf s'il y était déjà")
check(InputPipeline.axisMoved(from: -200, to: 200) && !InputPipeline.axisMoved(from: 32000, to: 32100), "signe changé gardé, bruit en butée écarté")

print(failed ? "ÉCHEC" : "tout passe")
exit(failed ? 1 : 0)
