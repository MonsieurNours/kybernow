// Règles de la manette à l'écran (ScreenPadLogic.swift) et du toucher de la
// vue de session (TouchRules.swift), vérifiées sans simulateur : voir run.sh.

import CoreGraphics
import Foundation

var failures = 0
var checks = 0

func check(_ ok: Bool, _ what: String, file: String = #file, line: Int = #line) {
    checks += 1
    if !ok {
        failures += 1
        print("ÉCHEC (ligne \(line)) : \(what)")
    }
}

func polar(_ m: Double, _ degrees: Double) -> (x: Double, y: Double) {
    let a = degrees * .pi / 180
    return (m * cos(a), m * sin(a))
}

// MARK: - Toucher : le clavier se replie, sans clic

check(TouchRules.tap(keyboardShown: true, barOpen: false, touchAsMouse: true) == .foldKeyboard, "clavier sorti : le toucher le replie")
check(TouchRules.tap(keyboardShown: true, barOpen: true, touchAsMouse: false) == .foldKeyboard, "clavier sorti, barre ouverte : d'abord le clavier")
check(TouchRules.tap(keyboardShown: false, barOpen: true, touchAsMouse: true) == .closeBar, "barre ouverte : le toucher la ferme")
check(TouchRules.tap(keyboardShown: false, barOpen: false, touchAsMouse: false) == .nothing, "manette affichée : pas de clic")
check(TouchRules.tap(keyboardShown: false, barOpen: false, touchAsMouse: true) == .click, "sinon : un clic")

// MARK: - Stick : valeurs

do {
    let r = PadStick.raw(offset: CGVector(dx: 200, dy: 0), travel: 50)
    check(abs(r.x - 1) < 1e-9 && r.y == 0, "déviation bornée au cercle")
    let up = PadStick.raw(offset: CGVector(dx: 0, dy: -25), travel: 50)
    check(abs(up.y - 0.5) < 1e-9, "vers le haut : y positif")
    check(PadStick.value((0.1, 0)) == (0, 0), "zone morte : zéro")
    let v = PadStick.value((1, 0))
    check(abs(v.x - 1) < 1e-9, "au bord : pleine déviation")
    let mid = PadStick.value((0.56, 0))
    check(mid.x > 0.45 && mid.x < 0.55, "entre les deux : proportionnel après la zone morte")
}

// MARK: - Stick : secteurs

check(PadStick.sector(x: 1, y: 0, previous: nil) == 0, "à droite : secteur 0")
check(PadStick.sector(x: 0, y: 1, previous: nil) == 2, "en haut : secteur 2")
check(PadStick.sector(x: -1, y: 0, previous: nil) == 4, "à gauche : secteur 4")
check(PadStick.sector(x: 0, y: -1, previous: nil) == 6, "en bas : secteur 6")
check(PadStick.sector(x: 1, y: -0.01, previous: nil) == 0, "juste sous l'horizontale : toujours à droite")
do {
    let p = polar(1, 25)
    check(PadStick.sector(x: p.x, y: p.y, previous: 0) == 0, "25° depuis la droite : l'hystérésis garde le secteur")
    check(PadStick.sector(x: p.x, y: p.y, previous: nil) == 1, "25° sans passé : secteur 1")
    let q = polar(1, 30)
    check(PadStick.sector(x: q.x, y: q.y, previous: 0) == 1, "30° : on change de secteur")
}

// MARK: - Stick : tics (sortie de la zone morte, changement de secteur, butée)

do {
    var feel = StickFeel()
    check(feel.update(polar(0.05, 0)) == [], "dans la zone morte : rien")
    check(feel.update(polar(0.2, 0)) == [.stickLeave], "le stick quitte le centre : un tic")
    check(feel.update(polar(0.5, 5)) == [], "même direction : rien")
    check(feel.update(polar(0.5, 26)) == [], "frontière du secteur : l'hystérésis évite le tic")
    check(feel.update(polar(0.5, 45)) == [.stickSector], "nouvelle direction : un tic très léger")
    check(feel.update(polar(0.6, 50)) == [], "rien de neuf")
    check(feel.update(polar(1, 50)) == [.stickEdge], "en butée : un tic")
    check(feel.update(polar(1, 52)) == [], "toujours en butée : pas de vibration continue")
    check(feel.update(polar(0.95, 52)) == [], "presque en butée : rien (hystérésis)")
    check(feel.update(polar(1, 52)) == [], "revient en butée sans l'avoir quittée : rien")
    check(feel.update(polar(0.8, 52)) == [], "quitte la butée : rien")
    check(feel.update(polar(1, 52)) == [.stickEdge], "nouvelle butée : un tic")
    check(feel.update(polar(1, 140)) == [.stickSector], "direction changée en butée : un tic de secteur seulement")
    check(feel.update(polar(0.11, 140)) == [], "près du centre, encore dehors : rien")
    check(feel.update(polar(0.13, 140)) == [], "tremblement au bord de la zone morte : pas de nouveau départ")
    check(feel.update(polar(0.05, 140)) == [], "retour au centre : rien")
    check(feel.update(polar(0.3, 270)) == [.stickLeave], "nouveau départ : un tic")
    check(feel.update(polar(1, 270)) == [.stickEdge], "droit en butée : un tic")
    feel.reset()
    check(feel.update(polar(1, 0)) == [.stickLeave, .stickEdge], "d'un coup au bord : départ puis butée")
}

// MARK: - Gâchettes

check(PadTrigger.value(along: 0) == PadTrigger.minimum, "doigt en haut : appui léger")
check(PadTrigger.value(along: 0.65) == 1, "aux deux tiers : bout de course")
check(PadTrigger.value(along: 1.4) == 1, "plus bas : toujours 1")
check(PadTrigger.value(along: 0.3) > PadTrigger.value(along: 0.1), "la valeur suit le doigt")
check(PadTrigger.minimum > 30.0 / 255, "un doigt posé dépasse le seuil de XInput")
do {
    var feel = TriggerFeel()
    check(feel.update(0) == [], "relâchée : rien")
    check(feel.update(0.3) == [.triggerStart], "début de course : clic léger")
    check(feel.update(0.6) == [], "en course : rien")
    check(feel.update(1) == [.triggerEnd], "bout de course : clic franc")
    check(feel.update(0.95) == [], "presque au bout : rien")
    check(feel.update(1) == [], "toujours au bout : rien")
    check(feel.update(0.8) == [], "remonte : rien")
    check(feel.update(1) == [.triggerEnd], "de nouveau au bout : clic franc")
    check(feel.update(0) == [], "lâchée : rien")
    check(feel.update(1) == [.triggerStart, .triggerEnd], "d'un coup au bout : les deux clics")
}

// MARK: - Croix

do {
    let none = PadDpad.directions(offset: CGVector(dx: 5, dy: 5), half: 54, previous: nil)
    check(none.0.isEmpty && none.sector == nil, "centre de la croix : rien")
    check(PadDpad.directions(offset: CGVector(dx: 0, dy: -40), half: 54, previous: nil).0 == [.up], "vers le haut")
    check(PadDpad.directions(offset: CGVector(dx: 40, dy: 0), half: 54, previous: nil).0 == [.right], "à droite")
    check(PadDpad.directions(offset: CGVector(dx: 30, dy: 30), half: 54, previous: nil).0 == [.right, .down], "diagonale : deux directions")
    check(PadDpad.directions(offset: CGVector(dx: -30, dy: -30), half: 54, previous: nil).0 == [.left, .up], "autre diagonale")
}

// MARK: - Stick enfoncé (L3, R3)

check(PadStickClick.isTap(duration: 0.12, maxDeviation: 0.1), "toucher bref et immobile : compte")
check(!PadStickClick.isTap(duration: 0.4, maxDeviation: 0.1), "trop long : non")
check(!PadStickClick.isTap(duration: 0.1, maxDeviation: 0.6), "a dévié : non")
check(PadStickClick.isClick(now: 10.3, lastTap: 10.0), "second toucher dans les 350 ms : enfoncé")
check(!PadStickClick.isClick(now: 10.5, lastTap: 10.0), "trop tard : non")
check(!PadStickClick.isClick(now: 10.0, lastTap: nil), "sans toucher avant : non")

// MARK: - Disposition

struct Screen {
    let name: String
    let size: CGSize
    let insets: PadInsets
    let compact: Bool
}

let screens = [
    Screen(name: "iPhone 16 Pro", size: CGSize(width: 874, height: 402), insets: PadInsets(top: 0, left: 62, bottom: 21, right: 62), compact: true),
    Screen(name: "iPhone SE", size: CGSize(width: 667, height: 375), insets: PadInsets(), compact: true),
    Screen(name: "iPhone 16 Pro Max", size: CGSize(width: 956, height: 440), insets: PadInsets(top: 0, left: 62, bottom: 21, right: 62), compact: true),
    Screen(name: "iPad Pro 13", size: CGSize(width: 1376, height: 1032), insets: PadInsets(top: 0, left: 0, bottom: 20, right: 0), compact: false),
    Screen(name: "iPad Pro 13 en portrait", size: CGSize(width: 1032, height: 1376), insets: PadInsets(top: 0, left: 0, bottom: 20, right: 0), compact: false),
    Screen(name: "iPad mini en portrait", size: CGSize(width: 744, height: 1133), insets: PadInsets(top: 0, left: 0, bottom: 20, right: 0), compact: false),
]

for screen in screens {
    var scales: [CGFloat] = []
    for size in ["small", "medium", "large"] {
        let factor = PadGeometry.factor(size) * (screen.compact ? 1 : 1.1)
        guard let g = PadGeometry.make(size: screen.size, insets: screen.insets, factor: factor, compactHeight: screen.compact) else {
            check(false, "\(screen.name), \(size) : pas de manette")
            continue
        }
        scales.append(g.scale)
        let safe = CGRect(x: screen.insets.left, y: screen.insets.top, width: screen.size.width - screen.insets.left - screen.insets.right,
                          height: screen.size.height - screen.insets.top - screen.insets.bottom)
        let handle = CGRect(x: screen.insets.left + safe.width / 2 - PadGeometry.handle.width / 2, y: 0,
                            width: PadGeometry.handle.width, height: PadGeometry.handle.height)
        let all = Array(g.items.values)
        check(all.count == PadControl.allCases.count, "\(screen.name), \(size) : toutes les commandes")
        for item in all {
            check(safe.contains(item.frame), "\(screen.name), \(size) : \(item.control) dans la zone sûre")
            check(!item.frame.intersects(handle), "\(screen.name), \(size) : \(item.control) laisse la poignée de la barre")
            check(item.hit.contains(item.frame), "\(screen.name), \(size) : zone de toucher de \(item.control) plus grande que le dessin")
            check(g.control(at: CGPoint(x: item.frame.midX, y: item.frame.midY), among: Set(PadControl.allCases)) == item.control,
                  "\(screen.name), \(size) : le centre de \(item.control) le touche")
        }
        for (i, a) in all.enumerated() {
            for b in all[(i + 1)...] {
                let apart: Bool
                if a.round, b.round {
                    let gap = hypot(a.frame.midX - b.frame.midX, a.frame.midY - b.frame.midY) - a.frame.width / 2 - b.frame.width / 2
                    apart = gap >= 2
                } else {
                    apart = !a.frame.insetBy(dx: -2, dy: -2).intersects(b.frame)
                }
                check(apart, "\(screen.name), \(size) : \(a.control) et \(b.control) ne se chevauchent pas")
            }
        }
        // Simple : les commandes cachées ne se touchent plus.
        let simple = Set(PadControl.allCases.filter(\.inSimple))
        if let rt = g.items[.rt] {
            check(g.control(at: CGPoint(x: rt.frame.midX, y: rt.frame.midY), among: simple) == nil, "\(screen.name), \(size) : en Simple, RT ne répond pas")
        }
        if let a = g.items[.a] {
            check(g.faceButton(at: CGPoint(x: a.frame.midX, y: a.frame.midY + 4)) == .a, "\(screen.name), \(size) : le pouce sur A")
        }
    }
    check(scales == scales.sorted() && Set(scales).count == scales.count, "\(screen.name) : Petite < Moyenne < Grande (\(scales))")
}

// Pupitre (barre toujours ouverte) : la manette descend sous la barre.
if let g = PadGeometry.make(size: CGSize(width: 874, height: 402), insets: PadInsets(top: 0, left: 62, bottom: 21, right: 62), factor: 1, compactHeight: true, topReserved: 76) {
    check(g.items.values.allSatisfy { $0.frame.minY >= 76 }, "pupitre : rien sous la barre")
} else {
    check(false, "pupitre : pas de manette")
}
check(PadGeometry.make(size: CGSize(width: 320, height: 240), insets: PadInsets(), factor: 1, compactHeight: true) == nil, "trop petit : pas de manette")

print(failures == 0 ? "\(checks) vérifications : tout est bon" : "\(failures) échec(s) sur \(checks)")
exit(failures == 0 ? 0 : 1)
