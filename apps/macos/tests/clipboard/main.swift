// Test hors ligne du presse-papiers partagé, sur un presse-papiers privé
// (celui du Mac n'est pas touché) : paquets kynput dans les deux sens.

import AppKit
import CKyber

var sent: [OpaquePointer] = []
let board = NSPasteboard(name: NSPasteboard.Name("fr.leslignesclaires.kybernow.test"))
board.clearContents()
let bridge = ClipboardBridge(board: board)
bridge.send = { if let p = $0 { sent.append(p) } }

func kind(_ p: OpaquePointer) -> String {
    var k = KYNPUT_CLIPBOARD_EVENT_KIND_CHANGE
    var text = false
    guard kynput_clipboard_event_from_packet(p, &k, &text) else { return "?" }
    switch k {
    case KYNPUT_CLIPBOARD_EVENT_KIND_CHANGE: return "Change"
    case KYNPUT_CLIPBOARD_EVENT_KIND_DATA: return "Data"
    case KYNPUT_CLIPBOARD_EVENT_KIND_DATA_REQUEST: return "DataRequest"
    default: return "DataUnavailable"
    }
}
func pump() { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
func check(_ ok: Bool, _ what: String) { print(ok ? "ok   " : "ÉCHEC", what); if !ok { exit(1) } }

// Ordinateur → Mac : annonce, le Mac demande, l'ordinateur répond.
bridge.received(kynput_clipboard_text_change_to_packet(KYNPUT_INPUT_TARGET_CLIENT))
check(sent.count == 1 && kind(sent[0]) == "DataRequest", "une annonce de l'ordinateur déclenche une demande")
let remote = "Bonjour depuis l'ordinateur — été 🎮"
let data = Array(remote.utf8).withUnsafeBufferPointer { kynput_clipboard_text_data_to_packet(KYNPUT_INPUT_TARGET_CLIENT, $0.baseAddress, $0.count)! }
bridge.received(data)
pump()
check(board.string(forType: .string) == remote, "le texte reçu arrive dans le presse-papiers (accents, emoji)")
sent.removeAll()
bridge.poll()
check(sent.isEmpty, "le texte écrit par l'app ne repart pas vers l'ordinateur")

// Mac → ordinateur : copie, annonce, demande, réponse.
board.clearContents()
board.setString("copié sur le Mac", forType: .string)
bridge.poll()
check(sent.count == 1 && kind(sent[0]) == "Change", "une copie sur le Mac est annoncée")
sent.removeAll()
bridge.received(kynput_clipboard_text_request_to_packet(KYNPUT_INPUT_TARGET_CLIENT))
pump()
var len = 0
let bytes = sent.first.flatMap { kynput_clipboard_text_from_packet($0, &len) }
let answer = bytes.map { String(bytes: UnsafeBufferPointer(start: $0, count: len), encoding: .utf8) } ?? nil
check(sent.count == 1 && kind(sent[0]) == "Data" && answer == "copié sur le Mac", "une demande de l'ordinateur reçoit le texte")

// Trop long : coupé à 60 Ko, sans casser un caractère.
sent.removeAll()
board.clearContents()
board.setString(String(repeating: "é", count: 40_000), forType: .string)
bridge.received(kynput_clipboard_text_request_to_packet(KYNPUT_INPUT_TARGET_CLIENT))
pump()
_ = sent.first.flatMap { kynput_clipboard_text_from_packet($0, &len) }
check(len <= 60 * 1024 && len % 2 == 0, "texte long coupé à \(len) octets (60 Ko au plus, UTF-8 intact)")

// Rien à coller.
sent.removeAll()
board.clearContents()
bridge.received(kynput_clipboard_text_request_to_packet(KYNPUT_INPUT_TARGET_CLIENT))
pump()
check(sent.count == 1 && kind(sent[0]) == "DataUnavailable", "presse-papiers vide : « indisponible »")
board.releaseGlobally()
