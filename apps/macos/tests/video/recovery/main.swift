// Reprise après une perte : flux à rafraîchissement progressif (comme NVENC
// dans Kyber), une image perdue à 120, une image clé forcée à 200 (comme
// celle que le serveur envoie sur demande, kyber-patches serveur 0005).

import Foundation

let aus = accessUnits(CommandLine.arguments[1], codec: 0x68323635)
let dec = VideoDecoder()
dec.setCodec(0x68323635)
var decoded: [Int] = []
var lost = 0, recovered = 0
var current = 0
dec.onFrame = { _ in decoded.append(current) }
dec.onLostReference = { lost += 1; print("  perte signalée à l'image \(current)") }
dec.onRecovered = { recovered += 1 }
for (i, au) in aus.enumerated() {
    current = i
    if i == 120 { continue } // perdue sur le réseau
    let key = i == 0 || i == 200
    au.withUnsafeBufferPointer { dec.decode($0, isConfig: false, isKey: key, pts: UInt64(i)) }
}
let after = decoded.filter { $0 > 120 }
func check(_ ok: Bool, _ what: String) { print(ok ? "ok   " : "ÉCHEC", what); if !ok { exit(1) } }
check(lost == 1, "la perte est signalée une seule fois (\(lost))")
check(after.first == 200, "rien n'est affiché entre la perte et l'image clé, reprise à \(after.first ?? -1)")
check(recovered == 1 && after.count == aus.count - 200, "tout est décodé après l'image clé (\(after.count)/\(aus.count - 200))")

// Image clé du début perdue entière, sans trou signalé (flux lourd en 4:4:4) :
// le décodeur doit en demander une au lieu d'attendre en silence.
let late = VideoDecoder()
late.setCodec(0x68323635)
var lateLost = 0, lateRecovered = 0, lateFirst: Int?
late.onFrame = { _ in if lateFirst == nil { lateFirst = current } }
late.onLostReference = { lateLost += 1 }
late.onRecovered = { lateRecovered += 1 }
for (i, au) in aus.enumerated() where i > 0 {
    current = i
    au.withUnsafeBufferPointer { late.decode($0, isConfig: false, isKey: i == 200, pts: UInt64(i)) }
}
check(lateLost == 1, "image clé du début perdue : une demande (\(lateLost))")
check(lateFirst == 200 && lateRecovered == 1, "reprise sur l'image clé suivante (\(lateFirst ?? -1))")
