// kybernow-display : écrans virtuels de KyberNow Station pour Mac
// (docs/station-macos.md, jalon 3), pilotés par Admin (`display/mac.rs`).
//
// Une requête JSON par ligne sur l'entrée standard, une réponse JSON par ligne
// sur la sortie : {"ok": true, ...} ou {"ok": false, "error": "..."}.
//
//   create     {"slot", "width", "height", "refresh", "hidpi"}  → {"id"}
//   remove     {"slot"}
//   isolate    {"screens": [{"id", "x", "y", "width", "height", "refresh", "hidpi"}], "keep_others"}
//              → {"screens": [{"id", "width", "height", "refresh", "x", "y"} | null], "disabled_others"}
//   restore    rend les écrans du Mac ; sans écran virtuel restant, l'agent s'arrête
//   state      {"ids"} → {"modes": [{"width", "height", "refresh"}], "displays", "others"}
//   set_mode   {"id", "width", "height", "refresh", "hidpi"} → {"width", "height", "refresh"}
//   displays   → {"displays": [{"id", "x", "y", "width", "height", "refresh", "primary"}]}
//   ping
//
// Le son de la session passe par les commandes ponctuelles `audio …`
// (audio.swift).
//
// Les écrans du Mac recopient l'écran virtuel principal pendant la session
// (`CGConfigureDisplayMirrorOfDisplay`) : seul l'écran virtuel reste actif, il
// devient l'écran principal, et la bascule est immédiate. Les éteindre
// (`CGSConfigureDisplayEnabled`, privé) n'était appliqué par macOS qu'avec
// retard, parfois à la sortie du programme seulement (essais du 03/11/2026).
//
// Filet de sécurité : les écrans virtuels vivent avec ce processus, et les
// changements de disposition sont faits « pour ce programme seulement »
// (kCGConfigureForAppOnly). Si Admin s'arrête (fin de l'entrée standard) ou si
// l'agent plante, macOS retire les écrans virtuels et rend les écrans du Mac
// à leur place.
//
// Les dimensions échangées sont en pixels. En HiDPI, l'écran prend le mode
// Retina (la moitié en points) : une interface lisible sur un téléphone ou un
// téléviseur 4K, comme macOS le choisit pour un écran 4K.

import CoreGraphics
import Foundation

setvbuf(stdout, nil, _IOLBF, 0)

struct Failure: Error { let message: String }

/// File des écrans virtuels : CoreGraphics y rend ses rappels pendant que
/// les requêtes attendent (jamais la file principale, que rien ne fait tourner).
let displayQueue = DispatchQueue(label: "fr.leslignesclaires.kybernow.display")

final class Agent {
    private var virtuals: [Int: CGVirtualDisplay] = [:]
    /// Retina ou non, fixé à la création (le numéro de produit le dit à Kyber).
    private var retina: [CGDirectDisplayID: Bool] = [:]
    /// Écrans du Mac mis en recopie par `isolate`, à rendre.
    private var mirroring: [CGDirectDisplayID] = []

    func handle(_ req: [String: Any]) throws -> [String: Any] {
        switch req["cmd"] as? String {
        case "create": return try create(req)
        case "remove":
            virtuals[try int(req, "slot")] = nil
            return [:]
        case "isolate": return try isolate(req)
        case "restore": return restore()
        case "state": return try state(req)
        case "set_mode": return try setMode(req)
        case "displays": return ["displays": physicalDisplays()]
        case "ping": return [:]
        default: throw Failure(message: "requête inconnue")
        }
    }

    var empty: Bool { virtuals.isEmpty }

    // MARK: création

    private func create(_ req: [String: Any]) throws -> [String: Any] {
        let slot = try int(req, "slot")
        let (w, h, hz) = (try int(req, "width"), try int(req, "height"), try int(req, "refresh"))
        let hidpi = req["hidpi"] as? Bool ?? false
        virtuals[slot] = nil
        let desc = CGVirtualDisplayDescriptor()
        desc.queue = displayQueue
        desc.name = slot == 0 ? "KyberNow" : "KyberNow \(slot + 1)"
        desc.maxPixelsWide = UInt32(w)
        desc.maxPixelsHigh = UInt32(h)
        // Taille physique : environ 110 points par pouce, ce que macOS
        // considère comme une densité normale (pas d'agrandissement imposé).
        let ppi = hidpi ? 220.0 : 110.0
        desc.sizeInMillimeters = CGSize(width: Double(w) / ppi * 25.4, height: Double(h) / ppi * 25.4)
        // Même numéro de série à chaque session : macOS retrouve les réglages de l'écran.
        desc.serialNum = 0x4B4E_0001 + UInt32(slot)
        // Fabricant « KN » ; produit : 0x4B31 en 1x, 0x4B32 en Retina. Kyber
        // (correctif server/macos/kyber-patches/0001) y lit l'échelle : un
        // processus déjà lancé ne voit pas le mode d'un écran apparu après lui.
        desc.productID = hidpi ? 0x4B32 : 0x4B31
        desc.vendorID = 0x4B4E
        desc.terminationHandler = { _, _ in FileHandle.standardError.write("écran virtuel retiré par le système\n".data(using: .utf8)!) }
        guard let display = CGVirtualDisplay(descriptor: desc) else {
            throw Failure(message: "macOS refuse de créer l'écran virtuel")
        }
        guard display.apply(settings(width: w, height: h, refresh: hz, hidpi: hidpi)) else {
            throw Failure(message: "macOS refuse le mode \(w) × \(h) à \(hz) Hz")
        }
        virtuals[slot] = display
        retina[display.displayID] = hidpi
        // L'écran met un instant à prendre son mode.
        waitUntil(seconds: 3) { CGDisplayCopyDisplayMode(display.displayID) != nil }
        return ["id": display.displayID]
    }

    // MARK: isolement

    private func isolate(_ req: [String: Any]) throws -> [String: Any] {
        guard let list = req["screens"] as? [[String: Any]], !list.isEmpty else { throw Failure(message: "aucun écran") }
        let keepOthers = req["keep_others"] as? Bool ?? false
        let ids = try list.map { CGDirectDisplayID(try int($0, "id")) }
        // Mode Retina d'abord : les positions sont en points.
        var config: CGDisplayConfigRef?
        CGBeginDisplayConfiguration(&config)
        var scales: [Int] = []
        for screen in list {
            let id = CGDirectDisplayID(try int(screen, "id"))
            let hidpi = screen["hidpi"] as? Bool ?? false
            if hidpi, let mode = retinaMode(id, width: try int(screen, "width"), height: try int(screen, "height")) {
                CGConfigureDisplayWithDisplayMode(config, id, mode, nil)
                scales.append(2)
            } else {
                scales.append(1)
            }
        }
        for (i, screen) in list.enumerated() {
            let id = ids[i]
            CGConfigureDisplayOrigin(config, id, Int32(try int(screen, "x") / scales[0]), Int32(try int(screen, "y") / scales[0]))
        }
        var others: [CGDirectDisplayID] = []
        if !keepOthers {
            others = onlineDisplays().filter { !ids.contains($0) && !isVirtual($0) }
            for id in others { CGConfigureDisplayMirrorOfDisplay(config, id, ids[0]) }
        }
        let err = CGCompleteDisplayConfiguration(config, .forAppOnly)
        guard err == .success else { throw Failure(message: "configuration des écrans refusée (\(err.rawValue))") }
        mirroring = Array(Set(mirroring + others))
        // Il ne reste que les écrans virtuels (ou ils sont actifs), le principal en (0, 0) ; 8 s au plus.
        let want = Set(ids)
        waitUntil(seconds: 8) {
            let active = Set(activeDisplays())
            return want.isSubset(of: active) && (keepOthers || active == want) && CGDisplayIsMain(ids[0]) != 0
        }
        let active = Set(activeDisplays())
        let screens: [Any] = ids.map { id in
            guard active.contains(id), let m = CGDisplayCopyDisplayMode(id) else { return NSNull() }
            let b = CGDisplayBounds(id)
            // Positions en pixels, comme celles demandées.
            return ["id": id, "width": m.pixelWidth, "height": m.pixelHeight, "refresh": Int(m.refreshRate.rounded()),
                    "x": Int(b.origin.x) * scales[0], "y": Int(b.origin.y) * scales[0]] as [String: Any]
        }
        let stillOn = keepOthers ? 0 : active.subtracting(want).count
        return ["screens": screens, "disabled_others": others.count - stillOn, "scales": scales.map { $0 * 100 }]
    }

    private func restore() -> [String: Any] {
        if !mirroring.isEmpty {
            var config: CGDisplayConfigRef?
            CGBeginDisplayConfiguration(&config)
            for id in mirroring { CGConfigureDisplayMirrorOfDisplay(config, id, kCGNullDirectDisplay) }
            CGCompleteDisplayConfiguration(config, .forAppOnly)
            mirroring.removeAll()
        }
        return ["exit": virtuals.isEmpty]
    }

    // MARK: veille et modes

    private func state(_ req: [String: Any]) throws -> [String: Any] {
        let ids = (req["ids"] as? [Any] ?? []).compactMap { ($0 as? NSNumber).map { CGDirectDisplayID($0.uint32Value) } }
        let active = activeDisplays()
        let modes: [[String: Int]] = ids.map { id in
            guard active.contains(id), let m = CGDisplayCopyDisplayMode(id) else { return ["width": 0, "height": 0, "refresh": 0] }
            return ["width": m.pixelWidth, "height": m.pixelHeight, "refresh": Int(m.refreshRate.rounded())]
        }
        let virtualIds = Set(virtuals.values.map(\.displayID))
        return ["modes": modes, "displays": active.count, "others": active.filter { !virtualIds.contains($0) }.count]
    }

    private func setMode(_ req: [String: Any]) throws -> [String: Any] {
        let id = CGDirectDisplayID(try int(req, "id"))
        let (w, h, hz) = (try int(req, "width"), try int(req, "height"), try int(req, "refresh"))
        guard let display = virtuals.values.first(where: { $0.displayID == id }) else { throw Failure(message: "écran virtuel inconnu") }
        // Retina comme à la création : Kyber le lit dans le numéro de produit.
        let hidpi = retina[id] ?? false
        // Un écran que d'autres recopient garde son mode : la recopie est
        // suspendue le temps du changement, puis remise.
        let mirrors = mirroring
        if !mirrors.isEmpty { configure { c in for m in mirrors { CGConfigureDisplayMirrorOfDisplay(c, m, kCGNullDirectDisplay) } } }
        let remirror = { if !mirrors.isEmpty { configure { c in for m in mirrors { CGConfigureDisplayMirrorOfDisplay(c, m, id) } } } }
        guard display.apply(settings(width: w, height: h, refresh: hz, hidpi: hidpi)) else {
            remirror()
            throw Failure(message: "macOS refuse le mode \(w) × \(h) à \(hz) Hz")
        }
        waitUntil(seconds: 3) { matchingMode(id, width: w, height: h, hidpi: hidpi) != nil }
        if let mode = matchingMode(id, width: w, height: h, hidpi: hidpi) {
            configure { c in CGConfigureDisplayWithDisplayMode(c, id, mode, nil) }
        }
        // Le nouveau mode n'est en place qu'une fois la recopie remise.
        remirror()
        waitUntil(seconds: 3) { CGDisplayCopyDisplayMode(id)?.pixelWidth == w }
        guard let m = CGDisplayCopyDisplayMode(id) else { return ["width": 0, "height": 0, "refresh": 0] }
        return ["width": m.pixelWidth, "height": m.pixelHeight, "refresh": Int(m.refreshRate.rounded())]
    }

    // MARK: écrans du Mac

    private func physicalDisplays() -> [[String: Any]] {
        activeDisplays().filter { !isVirtual($0) }.map { id in
            let b = CGDisplayBounds(id)
            let m = CGDisplayCopyDisplayMode(id)
            return ["id": id, "x": Int(b.origin.x), "y": Int(b.origin.y), "width": m?.pixelWidth ?? Int(b.width),
                    "height": m?.pixelHeight ?? Int(b.height), "refresh": Int((m?.refreshRate ?? 60).rounded()),
                    "primary": CGDisplayIsMain(id) != 0]
        }
    }

    private func isVirtual(_ id: CGDirectDisplayID) -> Bool {
        virtuals.values.contains { $0.displayID == id } || CGDisplayVendorNumber(id) == 0x4B4E
    }
}

/// Réglages d'un écran virtuel de `width` × `height` pixels. En HiDPI, le mode
/// est déclaré en points (la moitié) : macOS en fait alors le mode Retina sur
/// `width` × `height` pixels. Déclaré en pixels, macOS prendrait le double
/// (2868 points sur 5736 pixels), et son mode « moitié » n'est pas utilisable
/// pour le bureau (essais du 03/11/2026).
func settings(width w: Int, height h: Int, refresh hz: Int, hidpi: Bool) -> CGVirtualDisplaySettings {
    let s = CGVirtualDisplaySettings()
    s.hiDPI = hidpi ? 1 : 0
    let (mw, mh) = hidpi ? (w / 2, h / 2) : (w, h)
    s.modes = [CGVirtualDisplayMode(width: UInt(mw), height: UInt(mh), refreshRate: Double(hz))]
    return s
}

/// Mode Retina de l'écran : `width` × `height` pixels, la moitié en points.
func retinaMode(_ id: CGDirectDisplayID, width: Int, height: Int) -> CGDisplayMode? {
    matchingMode(id, width: width, height: height, hidpi: true)
}

/// Mode de `width` × `height` pixels, en Retina (la moitié en points) ou non.
func matchingMode(_ id: CGDirectDisplayID, width: Int, height: Int, hidpi: Bool) -> CGDisplayMode? {
    let options = [kCGDisplayShowDuplicateLowResolutionModes: kCFBooleanTrue] as CFDictionary
    let modes = CGDisplayCopyAllDisplayModes(id, options) as? [CGDisplayMode] ?? []
    return modes.first {
        $0.isUsableForDesktopGUI() && $0.pixelWidth == width && $0.pixelHeight == height && $0.width * (hidpi ? 2 : 1) == width
    }
}

/// Une configuration d'écrans « pour ce programme seulement ».
func configure(_ change: (CGDisplayConfigRef?) -> Void) {
    var config: CGDisplayConfigRef?
    CGBeginDisplayConfiguration(&config)
    change(config)
    CGCompleteDisplayConfiguration(config, .forAppOnly)
}

func onlineDisplays() -> [CGDirectDisplayID] {
    var ids = [CGDirectDisplayID](repeating: 0, count: 32)
    var n: UInt32 = 0
    CGGetOnlineDisplayList(32, &ids, &n)
    return Array(ids.prefix(Int(n)))
}

func activeDisplays() -> [CGDirectDisplayID] {
    var ids = [CGDirectDisplayID](repeating: 0, count: 32)
    var n: UInt32 = 0
    CGGetActiveDisplayList(32, &ids, &n)
    return Array(ids.prefix(Int(n)))
}

func waitUntil(seconds: Double, _ done: () -> Bool) {
    let deadline = Date().addingTimeInterval(seconds)
    while !done() && Date() < deadline { usleep(100_000) }
}

func int(_ dict: [String: Any], _ key: String) throws -> Int {
    guard let n = dict[key] as? NSNumber else { throw Failure(message: "« \(key) » manquant") }
    return n.intValue
}

func reply(_ body: [String: Any]) {
    let data = (try? JSONSerialization.data(withJSONObject: body)) ?? Data("{\"ok\":false}".utf8)
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data("\n".utf8))
}

// Commandes ponctuelles du son (audio.swift) : une réponse, puis la fin.
if CommandLine.arguments.count > 1, CommandLine.arguments[1] == "audio" {
    do {
        var body = try audioCommand(Array(CommandLine.arguments.dropFirst(2)))
        body["ok"] = true
        reply(body)
        exit(0)
    } catch let f as Failure {
        reply(["ok": false, "error": f.message])
    } catch {
        reply(["ok": false, "error": "\(error)"])
    }
    exit(1)
}

let agent = Agent()
while let line = readLine() {
    guard let data = line.data(using: .utf8), let req = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
        reply(["ok": false, "error": "requête illisible"])
        continue
    }
    do {
        var body = try agent.handle(req)
        let exitAfter = body.removeValue(forKey: "exit") as? Bool ?? false
        body["ok"] = true
        reply(body)
        if exitAfter { exit(0) }
    } catch let f as Failure {
        reply(["ok": false, "error": f.message])
    } catch {
        reply(["ok": false, "error": "\(error)"])
    }
}
// Admin s'est arrêté : macOS défait tout en même temps que ce processus.
exit(0)
