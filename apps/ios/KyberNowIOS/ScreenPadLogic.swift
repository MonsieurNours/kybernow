// Manette à l'écran, sans UIKit : où se trouve chaque commande, ce que vaut
// un stick, une gâchette ou la croix sous le doigt, et quand le retour
// haptique donne un tic. La vue (ScreenPad.swift) dessine et envoie ;
// apps/ios/tests/screen-pad/run.sh vérifie ces règles.

import CoreGraphics
import Foundation

/// Une commande de la manette à l'écran.
enum PadControl: String, CaseIterable, Hashable {
    case leftStick, rightStick, dpad, a, b, x, y, lb, rb, lt, rt, view, guide, menu, toggle

    /// Dans la disposition Simple : le stick gauche, la croix, A, B, X, Y et
    /// Menu (et le bouton qui revient à la Complète).
    var inSimple: Bool {
        switch self {
        case .leftStick, .dpad, .a, .b, .x, .y, .menu, .toggle: return true
        default: return false
        }
    }

    static let face: [PadControl] = [.a, .b, .x, .y]

    var isFace: Bool { Self.face.contains(self) }
    var isStick: Bool { self == .leftStick || self == .rightStick }
    var isTrigger: Bool { self == .lt || self == .rt }
}

/// Marges sûres de l'écran (encoche, Dynamic Island, barre de gestes).
struct PadInsets: Equatable {
    var top: CGFloat = 0, left: CGFloat = 0, bottom: CGFloat = 0, right: CGFloat = 0
}

/// Disposition fixe, pour les deux pouces en paysage (comme une manette
/// Xbox) : à gauche, les gâchette et bouton de tranche en haut, le stick
/// puis la croix plus bas et vers l'intérieur ; à droite, A B X Y puis le
/// stick droit ; en haut au centre, de part et d'autre de la poignée de la
/// barre, Affichage, Guide, Menu et le bouton de disposition. Les commandes
/// communes aux deux dispositions gardent leur place : passer de l'une à
/// l'autre ne déplace rien sous les doigts.
struct PadGeometry: Equatable {
    struct Item: Equatable {
        let control: PadControl
        /// Ce qui est dessiné.
        let frame: CGRect
        let round: Bool
        /// Ce qui se touche : le dessin et une marge autour.
        let hit: CGRect
    }

    let items: [PadControl: Item]
    let scale: CGFloat
    /// Course du doigt sur un stick, jusqu'à la pleine déviation.
    let stickTravel: CGFloat

    // Mesures à l'échelle 1 (points), celles d'un iPhone en taille Moyenne.
    static let margin: CGFloat = 14
    static let triggerSize = CGSize(width: 88, height: 50)
    static let bumperSize = CGSize(width: 88, height: 34)
    static let shoulderGap: CGFloat = 6
    static let stickRadius: CGFloat = 56
    static let dpadSize: CGFloat = 108
    static let faceDiameter: CGFloat = 50
    static let faceSpread: CGFloat = 44
    static let sideGap: CGFloat = 10
    static let smallSize = CGSize(width: 44, height: 30)
    static let guideDiameter: CGFloat = 36
    static let toggleDiameter: CGFloat = 30
    /// Poignée de la barre de session (languette en haut au centre).
    static let handle = CGSize(width: 88, height: 26)

    /// Facteurs des tailles Petite, Moyenne, Grande.
    static func factor(_ size: String) -> CGFloat {
        switch size {
        case "small": return 0.84
        case "large": return 1.16
        default: return 1
        }
    }

    /// Le stick gauche (et A B X Y) au-dessus de la croix, en part de leur écart.
    static let stack: CGFloat = 0.8

    /// Hauteur qu'il faut à la manette (bas compris), à l'échelle 1.
    private static var neededHeight: CGFloat {
        let shoulders = 6 + triggerSize.height + shoulderGap + bumperSize.height
        let upper = max(stickRadius, faceSpread + faceDiameter / 2)
        let sticks = dpadSize / 2 + (stickRadius + dpadSize / 2) * stack + upper
        return shoulders + 10 + sticks + margin
    }

    /// Largeur qu'il faut aux deux côtés du bas, à l'échelle 1.
    private static var neededWidth: CGFloat {
        let left = 2 * stickRadius + sideGap + dpadSize
        let right = 2 * (faceSpread + faceDiameter / 2) + sideGap + 2 * stickRadius
        return 2 * margin + left + right + 40
    }

    /// `factor` : la taille choisie (et ×1,1 sur iPad) ; ramenée à ce que
    /// l'écran contient, jamais sous 0,6 (sinon nil : pas de manette).
    /// `topReserved` : la barre de session est toujours ouverte (pupitre).
    static func make(size: CGSize, insets: PadInsets, factor: CGFloat, compactHeight: Bool, topReserved: CGFloat = 0) -> PadGeometry? {
        let w = size.width - insets.left - insets.right
        let top = max(insets.top, topReserved)
        let h = size.height - top - insets.bottom
        guard w > 0, h > 0 else { return nil }
        let s = min(factor, w / neededWidth, h / neededHeight)
        guard s >= 0.6 else { return nil }
        // iPhone (ou fenêtre basse) : la manette prend toute la hauteur ;
        // iPad : elle se pose en bas, à portée des pouces.
        let boxHeight = compactHeight ? h : min(h, neededHeight * s * 1.08)
        let boxTop = top + h - boxHeight
        let bottom = size.height - insets.bottom - margin * s
        let left = insets.left + margin * s
        let right = size.width - insets.right - margin * s
        let cx = insets.left + w / 2

        var items: [PadControl: Item] = [:]
        func put(_ c: PadControl, _ frame: CGRect, round: Bool, slack: CGFloat) {
            items[c] = Item(control: c, frame: frame, round: round, hit: frame.insetBy(dx: -slack * s, dy: -slack * s))
        }
        func rect(center: CGPoint, _ size: CGSize) -> CGRect {
            CGRect(x: center.x - size.width / 2, y: center.y - size.height / 2, width: size.width, height: size.height)
        }

        // Tranches : gâchette au plus haut, bouton de tranche dessous.
        let trigger = CGSize(width: triggerSize.width * s, height: triggerSize.height * s)
        let bumper = CGSize(width: bumperSize.width * s, height: bumperSize.height * s)
        let shoulderTop = boxTop + 6 * s
        put(.lt, CGRect(x: left, y: shoulderTop, width: trigger.width, height: trigger.height), round: false, slack: 12)
        put(.rt, CGRect(x: right - trigger.width, y: shoulderTop, width: trigger.width, height: trigger.height), round: false, slack: 12)
        let bumperTop = shoulderTop + trigger.height + shoulderGap * s
        put(.lb, CGRect(x: left, y: bumperTop, width: bumper.width, height: bumper.height), round: false, slack: 12)
        put(.rb, CGRect(x: right - bumper.width, y: bumperTop, width: bumper.width, height: bumper.height), round: false, slack: 12)

        // Gauche : la croix en bas, le stick au-dessus, vers l'extérieur.
        let r = stickRadius * s, d = dpadSize * s
        let dpadCenter = CGPoint(x: left + 2 * r + sideGap * s + d / 2, y: bottom - d / 2)
        put(.dpad, rect(center: dpadCenter, CGSize(width: d, height: d)), round: false, slack: 16)
        let stickY = dpadCenter.y - (r + d / 2) * stack
        put(.leftStick, rect(center: CGPoint(x: left + r, y: stickY), CGSize(width: 2 * r, height: 2 * r)), round: true, slack: 24)

        // Droite : A B X Y à l'extérieur, le stick droit en bas, vers l'intérieur.
        let spread = faceSpread * s, fd = faceDiameter * s
        let face = CGPoint(x: right - spread - fd / 2, y: stickY)
        let faceSize = CGSize(width: fd, height: fd)
        put(.a, rect(center: CGPoint(x: face.x, y: face.y + spread), faceSize), round: true, slack: 12)
        put(.b, rect(center: CGPoint(x: face.x + spread, y: face.y), faceSize), round: true, slack: 12)
        put(.x, rect(center: CGPoint(x: face.x - spread, y: face.y), faceSize), round: true, slack: 12)
        put(.y, rect(center: CGPoint(x: face.x, y: face.y - spread), faceSize), round: true, slack: 12)
        let rightStick = CGPoint(x: right - 2 * (spread + fd / 2) - sideGap * s - r, y: bottom - r)
        put(.rightStick, rect(center: rightStick, CGSize(width: 2 * r, height: 2 * r)), round: true, slack: 24)

        // En haut au centre : Affichage, Guide, Menu, bouton de disposition.
        // Quand la manette monte jusqu'en haut de l'écran, la poignée de la
        // barre garde sa place au milieu.
        let small = CGSize(width: smallSize.width * s, height: smallSize.height * s)
        let rowY = boxTop + 6 * s + small.height / 2 + (top < 1 ? 2 : 0)
        let handleGap = boxTop < handle.height + 20 ? handle.width / 2 + 18 : 0
        let step = 56 * s
        let guide = CGSize(width: guideDiameter * s, height: guideDiameter * s)
        let toggle = CGSize(width: toggleDiameter * s, height: toggleDiameter * s)
        if handleGap > 0 {
            // Poignée au milieu : Affichage et Guide à gauche, Menu et le bouton à droite.
            put(.guide, rect(center: CGPoint(x: cx - handleGap - guide.width / 2, y: rowY), guide), round: true, slack: 10)
            put(.view, rect(center: CGPoint(x: cx - handleGap - guide.width / 2 - step, y: rowY), small), round: false, slack: 10)
            put(.menu, rect(center: CGPoint(x: cx + handleGap + small.width / 2, y: rowY), small), round: false, slack: 10)
            put(.toggle, rect(center: CGPoint(x: cx + handleGap + small.width / 2 + step, y: rowY), toggle), round: true, slack: 8)
        } else {
            put(.view, rect(center: CGPoint(x: cx - step, y: rowY), small), round: false, slack: 10)
            put(.guide, rect(center: CGPoint(x: cx, y: rowY), guide), round: true, slack: 10)
            put(.menu, rect(center: CGPoint(x: cx + step, y: rowY), small), round: false, slack: 10)
            put(.toggle, rect(center: CGPoint(x: cx + 2 * step, y: rowY), toggle), round: true, slack: 8)
        }
        return PadGeometry(items: items, scale: s, stickTravel: r * 0.78)
    }

    /// La commande sous le doigt : parmi celles dont la zone de toucher
    /// contient le point, la plus proche de son dessin (rapporté à sa taille).
    func control(at p: CGPoint, among visible: Set<PadControl>) -> PadControl? {
        var best: (PadControl, CGFloat)?
        for (c, item) in items where visible.contains(c) && item.hit.contains(p) {
            let f = item.frame
            let dx = max(abs(p.x - f.midX) - f.width / 2, 0) / max(f.width, 1)
            let dy = max(abs(p.y - f.midY) - f.height / 2, 0) / max(f.height, 1)
            var score = hypot(dx, dy)
            // Dans le dessin : la plus proche de son centre.
            if score == 0 { score = -1 + hypot((p.x - f.midX) / f.width, (p.y - f.midY) / f.height) }
            if best == nil || score < best!.1 { best = (c, score) }
        }
        return best?.0
    }

    /// Le bouton A, B, X ou Y le plus proche du doigt, s'il est dans la zone
    /// de l'un d'eux (le pouce glisse d'un bouton à l'autre).
    func faceButton(at p: CGPoint) -> PadControl? {
        PadControl.face
            .compactMap { c in items[c].map { (c, $0) } }
            .filter { $0.1.hit.contains(p) }
            .min { hypot(p.x - $0.1.frame.midX, p.y - $0.1.frame.midY) < hypot(p.x - $1.1.frame.midX, p.y - $1.1.frame.midY) }?
            .0
    }
}

// MARK: - Sticks

enum PadStick {
    /// Zone morte : en deçà, le stick est au centre.
    static let deadZone = 0.12

    /// Déviation brute (-1…1, vers le haut positif, bornée au cercle) d'un
    /// doigt décalé de `offset` (points de l'écran, vers le bas positif).
    static func raw(offset: CGVector, travel: CGFloat) -> (x: Double, y: Double) {
        guard travel > 0 else { return (0, 0) }
        var x = Double(offset.dx / travel), y = Double(-offset.dy / travel)
        let m = hypot(x, y)
        if m > 1 { x /= m; y /= m }
        return (x, y)
    }

    /// Valeur envoyée : zéro dans la zone morte, puis de 0 à 1 jusqu'au bord.
    static func value(_ raw: (x: Double, y: Double)) -> (x: Double, y: Double) {
        let m = hypot(raw.x, raw.y)
        guard m > deadZone else { return (0, 0) }
        let k = (m - deadZone) / (1 - deadZone) / m
        return (raw.x * k, raw.y * k)
    }

    /// Secteur (0 : à droite, puis dans le sens inverse des aiguilles d'une
    /// montre, 8 secteurs de 45°), avec 6° d'hystérésis autour du précédent :
    /// un doigt posé sur une frontière ne fait pas cliqueter.
    static func sector(x: Double, y: Double, previous: Int?, hysteresis: Double = 6) -> Int {
        var angle = atan2(y, x) * 180 / .pi
        if angle < 0 { angle += 360 }
        let raw = Int(((angle + 22.5) / 45).rounded(.down)) % 8
        guard let previous, previous != raw else { return raw }
        // Distance angulaire au centre du secteur précédent.
        var delta = abs(angle - Double(previous) * 45)
        if delta > 180 { delta = 360 - delta }
        return delta <= 22.5 + hysteresis ? previous : raw
    }
}

/// Tics du retour haptique.
enum PadTick: Equatable {
    /// Bouton, croix, stick enfoncé (L3, R3) : un clic net.
    case press
    /// Gâchette : début de la course, puis bout de la course.
    case triggerStart, triggerEnd
    /// Stick : il quitte le centre, change de direction (8), arrive en butée.
    case stickLeave, stickSector, stickEdge
}

/// Suivi d'un stick pour ses tics : sortie de la zone morte, changement de
/// secteur (hors de la zone morte), butée ; jamais de vibration continue.
struct StickFeel: Equatable {
    private(set) var out = false
    private(set) var sector: Int?
    private(set) var atEdge = false

    /// Retour au centre sous 3/4 de la zone morte : un doigt qui tremble au
    /// bord de la zone ne cliquette pas.
    static let returnZone = PadStick.deadZone * 0.75
    static let edgeIn = 0.97, edgeOut = 0.9

    /// `raw` : la déviation brute (`PadStick.raw`).
    mutating func update(_ raw: (x: Double, y: Double)) -> [PadTick] {
        let m = hypot(raw.x, raw.y)
        var ticks: [PadTick] = []
        if !out {
            guard m >= PadStick.deadZone else { return [] }
            out = true
            sector = PadStick.sector(x: raw.x, y: raw.y, previous: nil)
            ticks.append(.stickLeave)
        } else {
            if m < Self.returnZone {
                reset()
                return []
            }
            let next = PadStick.sector(x: raw.x, y: raw.y, previous: sector)
            if next != sector {
                sector = next
                ticks.append(.stickSector)
            }
        }
        if !atEdge, m >= Self.edgeIn {
            atEdge = true
            ticks.append(.stickEdge)
        } else if atEdge, m < Self.edgeOut {
            atEdge = false
        }
        return ticks
    }

    mutating func reset() {
        out = false
        sector = nil
        atEdge = false
    }
}

// MARK: - Gâchettes

enum PadTrigger {
    /// Valeur la plus faible d'un doigt posé : au-dessus du seuil de XInput
    /// (30/255) et de celui de la plupart des jeux.
    static let minimum = 0.3
    /// Part de la hauteur où la gâchette est au bout de sa course.
    static let fullAt = 0.65

    /// Valeur d'une gâchette touchée à `along` de sa hauteur (0 en haut, 1 en
    /// bas) : plus le doigt est bas, plus elle est enfoncée ; elle suit le
    /// doigt qui glisse.
    static func value(along: Double) -> Double {
        let t = min(1, max(0, along))
        return minimum + (1 - minimum) * min(1, t / fullAt)
    }
}

/// Tics d'une gâchette : clic léger au début de la course, clic franc au bout.
struct TriggerFeel: Equatable {
    private(set) var pressed = false
    private(set) var atEnd = false

    mutating func update(_ value: Double) -> [PadTick] {
        guard value > 0 else {
            pressed = false
            atEnd = false
            return []
        }
        var ticks: [PadTick] = []
        if !pressed {
            pressed = true
            ticks.append(.triggerStart)
        }
        if !atEnd, value >= 0.99 {
            atEnd = true
            ticks.append(.triggerEnd)
        } else if atEnd, value < 0.9 {
            atEnd = false
        }
        return ticks
    }
}

// MARK: - Croix

enum PadDirection: String, CaseIterable, Hashable {
    case up, down, left, right
}

enum PadDpad {
    /// Le centre de la croix ne presse rien.
    static let center = 0.22

    /// Directions pressées par un doigt décalé de `offset` (points, vers le
    /// bas positif) sur une croix de demi-largeur `half` : huit secteurs, les
    /// diagonales pressent deux directions.
    static func directions(offset: CGVector, half: CGFloat, previous: Int?) -> (Set<PadDirection>, sector: Int?) {
        guard half > 0 else { return ([], nil) }
        let x = Double(offset.dx / half), y = Double(-offset.dy / half)
        guard hypot(x, y) >= center else { return ([], nil) }
        let sector = PadStick.sector(x: x, y: y, previous: previous)
        let set: Set<PadDirection> = switch sector {
        case 0: [.right]
        case 1: [.right, .up]
        case 2: [.up]
        case 3: [.up, .left]
        case 4: [.left]
        case 5: [.left, .down]
        case 6: [.down]
        default: [.down, .right]
        }
        return (set, sector)
    }
}

// MARK: - Stick enfoncé

/// L3 et R3 : toucher deux fois le stick (un toucher court, puis un second
/// dans les 350 ms) l'enfonce tant que le doigt reste.
enum PadStickClick {
    static let tapMax: TimeInterval = 0.25
    static let gapMax: TimeInterval = 0.35
    static let tapTravel = 0.3

    /// Le toucher précédent (`lastTap` : son lever, s'il était court et
    /// presque immobile) fait de celui qui commence à `now` un appui.
    static func isClick(now: TimeInterval, lastTap: TimeInterval?) -> Bool {
        guard let lastTap else { return false }
        return now - lastTap <= gapMax && now >= lastTap
    }

    /// Un toucher compte comme « court » : bref, et qui n'a presque pas dévié.
    static func isTap(duration: TimeInterval, maxDeviation: Double) -> Bool {
        duration <= tapMax && maxDeviation <= tapTravel
    }
}
