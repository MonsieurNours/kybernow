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
//   set_hdr    {"id", "on"} → {"supported", "before", "active"} (mode du même écran, en HDR ou en SDR)
//   hdr        {"id"} → {"supported", "active"}
//   ping
//
// Le son de la session passe par les commandes ponctuelles `audio …`
// (audio.swift), l'app du début de la session par `apps`, `launch`, `close`
// et `locked` (apps.swift).
//
// Les écrans du Mac sont éteints pendant la session, comme sous Windows :
// l'écran virtuel principal devient l'écran principal (en (0, 0), les écrans
// du Mac décalés à sa droite), puis chaque écran du Mac est éteint
// (`CGSConfigureDisplayEnabled`, privé), **un par configuration**, en
// attendant qu'il le soit : plusieurs à la fois, macOS les éteint en retard,
// un à un et dans le désordre, et un écran qui perd ainsi son signal (l'Asus
// XG32UQ de l'auteur) ne revient qu'en le débranchant. Un par un, c'est fait
// en moins d'une demi-seconde chacun ; le rallumage aussi, puis la
// disposition d'avant (principal, places) est remise (essais du 03/11/2026).
//
// Filet de sécurité : les écrans virtuels vivent avec ce processus. Les écrans
// éteints, eux, le restent même s'il s'arrête : la liste et la disposition
// d'avant sont écrites dans `~/Library/Application Support/KyberNow/
// display-off.json` avant de les éteindre, et rallumées au démarrage de
// l'agent suivant (ou par `kybernow-display recover`, qu'Admin lance à son
// démarrage) ; à la fin de l'entrée standard (Admin arrêté), l'agent les
// rallume avant de partir.
//
// Les dimensions échangées sont en pixels. En HiDPI, l'écran prend le mode
// Retina (la moitié en points) : une interface lisible sur un téléphone ou une
// tablette. Dès 4K, l'écran a toutes les échelles de macOS, comme un vrai
// écran 4K (Réglages › Moniteurs : du plus grand texte au plus d'espace) :
// macOS y met celle qu'il recommande (« 2560 × 1440 ») ou celle choisie la
// dernière fois à cette définition ; l'image reste à la définition de
// l'appareil (`virtual-displays.json`, correctif Kyber Mac 0014).

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
    /// Mode de chaque écran virtuel (pixels, Hz) et HDR ou non.
    private var modes: [CGDirectDisplayID: (w: Int, h: Int, hz: Int, hdr: Bool)] = [:]
    /// Écrans à toutes les échelles de macOS (4K et plus, voir `scaleList`) :
    /// le bureau suit l'échelle choisie dans Réglages, l'image garde la
    /// définition de l'appareil.
    private var scaled: Set<CGDirectDisplayID> = []
    /// Écrans du Mac éteints par `isolate`, et leur place d'avant.
    private var switchedOff: OffState?

    func handle(_ req: [String: Any]) throws -> [String: Any] {
        switch req["cmd"] as? String {
        case "create": return try create(req)
        case "remove":
            let slot = try int(req, "slot")
            // Le dernier écran virtuel : les écrans du Mac reviennent d'abord.
            // Sans aucun écran, même un instant, macOS allume l'écran intégré
            // et ne rallume plus les autres (fin de session du 03/11/2026 :
            // Admin retire les écrans virtuels avant `restore`).
            if switchedOff != nil, virtuals.keys.allSatisfy({ $0 == slot }) {
                _ = restore()
            }
            virtuals[slot] = nil
            writePanels()
            return [:]
        case "isolate": return try isolate(req)
        case "restore": return restore()
        case "state": return try state(req)
        case "set_mode": return try setMode(req)
        case "displays": return ["displays": physicalDisplays()]
        case "set_hdr": return try setHDR(req)
        case "hdr":
            let id = CGDirectDisplayID(try int(req, "id"))
            return ["supported": virtuals.values.contains { $0.displayID == id }, "active": hdrActive(id)]
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
        let allScales = hidpi && w >= scaledFromWidth
        // Toutes les échelles : macOS dessine jusqu'au double de la définition
        // de l'appareil (« 2560 × 1440 » sur un écran 4K : 5120 × 2880).
        desc.maxPixelsWide = UInt32(allScales ? 2 * w : w)
        desc.maxPixelsHigh = UInt32(allScales ? 2 * h : h)
        // Taille physique : environ 110 points par pouce, ce que macOS
        // considère comme une densité normale (pas d'agrandissement imposé) ;
        // un écran 4K se présente comme un 27 pouces, macOS y recommande
        // alors « 2560 × 1440 » (essai du 04/11/2026).
        let ppi = allScales ? 163.0 : hidpi ? 220.0 : 110.0
        desc.sizeInMillimeters = CGSize(width: Double(w) / ppi * 25.4, height: Double(h) / ppi * 25.4)
        // Même numéro de série à chaque session : macOS retrouve les réglages
        // de l'écran. À toutes les échelles : un numéro par définition, pour
        // que macOS retienne l'échelle choisie pour chacune.
        desc.serialNum = allScales ? scaledSerial(slot: slot, width: w, height: h) : 0x4B4E_0001 + UInt32(slot)
        // Fabricant « KN » ; produit : 0x4B31 en 1x, 0x4B32 en Retina. Kyber
        // (correctif server/macos/kyber-patches/0001) y lit l'échelle : un
        // processus déjà lancé ne voit pas le mode d'un écran apparu après lui.
        desc.productID = hidpi ? 0x4B32 : 0x4B31
        desc.vendorID = 0x4B4E
        desc.terminationHandler = { _, _ in FileHandle.standardError.write("écran virtuel retiré par le système\n".data(using: .utf8)!) }
        guard let display = CGVirtualDisplay(descriptor: desc) else {
            throw Failure(message: "macOS refuse de créer l'écran virtuel")
        }
        virtuals[slot] = display
        retina[display.displayID] = hidpi
        modes[display.displayID] = (w, h, hz, false)
        if allScales { scaled.insert(display.displayID) } else { scaled.remove(display.displayID) }
        // La définition pour Kyber avant que l'écran s'allume : Kyber le
        // découvre dès qu'il apparaît.
        writePanels()
        guard display.apply(settings(width: w, height: h, refresh: hz, hidpi: hidpi, allScales: allScales)) else {
            virtuals[slot] = nil
            writePanels()
            throw Failure(message: "macOS refuse le mode \(w) × \(h) à \(hz) Hz")
        }
        // L'écran met un instant à prendre son mode (macOS y met l'échelle
        // retenue pour cet écran, sinon celle qu'il recommande).
        waitUntil(seconds: 3) { CGDisplayCopyDisplayMode(display.displayID) != nil }
        return ["id": display.displayID]
    }

    // MARK: isolement

    private func isolate(_ req: [String: Any]) throws -> [String: Any] {
        guard let list = req["screens"] as? [[String: Any]], !list.isEmpty else { throw Failure(message: "aucun écran") }
        let keepOthers = req["keep_others"] as? Bool ?? false
        let ids = try list.map { CGDirectDisplayID(try int($0, "id")) }
        if switchedOff == nil {
            previousOrigins = Dictionary(uniqueKeysWithValues: onlineDisplays().filter { !isVirtual($0) }.map { ($0, CGDisplayBounds($0).origin) })
        }
        // Mode Retina d'abord : les positions sont en points.
        var config: CGDisplayConfigRef?
        CGBeginDisplayConfiguration(&config)
        var scales: [Double] = []
        for screen in list {
            let id = CGDirectDisplayID(try int(screen, "id"))
            let hidpi = screen["hidpi"] as? Bool ?? false
            if scaled.contains(id) {
                // L'échelle de macOS reste (celle retenue ou recommandée).
                scales.append(scaleOf(id))
            } else if hidpi, let mode = retinaMode(id, width: try int(screen, "width"), height: try int(screen, "height")) {
                CGConfigureDisplayWithDisplayMode(config, id, mode, nil)
                scales.append(2)
            } else {
                scales.append(1)
            }
        }
        for (i, screen) in list.enumerated() {
            let id = ids[i]
            CGConfigureDisplayOrigin(config, id, Int32((Double(try int(screen, "x")) / scales[0]).rounded()),
                                     Int32((Double(try int(screen, "y")) / scales[0]).rounded()))
        }
        var others: [CGDirectDisplayID] = []
        if !keepOthers {
            others = onlineDisplays().filter { !ids.contains($0) && !isVirtual($0) && !(switchedOff?.ids.contains($0) ?? false) }
            // Les écrans du Mac collés à droite des écrans virtuels : l'écran
            // virtuel principal, en (0, 0), devient l'écran principal.
            var right: Int32 = 0
            for (i, screen) in list.enumerated() {
                let x = Double((try? int(screen, "x")) ?? 0) / scales[0]
                let w = Double((try? int(screen, "width")) ?? 0) / scales[i]
                right = max(right, Int32((x + w).rounded()))
            }
            // Noté avant de toucher à quoi que ce soit (voir l'en-tête).
            noteOff(others)
            for id in others {
                CGConfigureDisplayOrigin(config, id, right, 0)
                right += Int32(CGDisplayBounds(id).width)
            }
        }
        let err = CGCompleteDisplayConfiguration(config, keepOthers ? .forAppOnly : .forSession)
        guard err == .success else { throw Failure(message: "configuration des écrans refusée (\(err.rawValue))") }
        if !keepOthers {
            waitUntil(seconds: 4) { CGDisplayIsMain(ids[0]) != 0 }
            switchOff(others)
        }
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
            // Positions en pixels, comme celles demandées ; à toutes les
            // échelles, la définition de l'appareil.
            let (w, h) = panel(id) ?? (m.pixelWidth, m.pixelHeight)
            return ["id": id, "width": w, "height": h, "refresh": Int(m.refreshRate.rounded()),
                    "x": Int((b.origin.x * scales[0]).rounded()), "y": Int((b.origin.y * scales[0]).rounded())] as [String: Any]
        }
        let stillOn = keepOthers ? 0 : active.subtracting(want).count
        return ["screens": screens, "disabled_others": others.count - stillOn, "scales": scales.map { Int(($0 * 100).rounded()) }]
    }

    /// Note les écrans `ids` comme éteints, avec leur place d'avant (celle
    /// de la première configuration de la session), sur disque aussi.
    private func noteOff(_ ids: [CGDirectDisplayID]) {
        guard !ids.isEmpty else { return }
        var state = switchedOff ?? OffState(ids: [], origins: [:])
        for id in ids where state.origins[id] == nil {
            state.origins[id] = previousOrigins[id] ?? CGDisplayBounds(id).origin
        }
        state.ids.append(contentsOf: ids.filter { !state.ids.contains($0) })
        switchedOff = state
        state.save()
    }

    /// Éteint les écrans `ids` un par un (voir l'en-tête).
    private func switchOff(_ ids: [CGDirectDisplayID]) {
        for id in ids { setDisplayEnabled(id, false) }
    }

    /// Places des écrans du Mac avant la première configuration de la session.
    private var previousOrigins: [CGDirectDisplayID: CGPoint] = [:]

    private func restore() -> [String: Any] {
        if let state = switchedOff {
            state.switchOn()
            switchedOff = nil
        }
        return ["exit": virtuals.isEmpty]
    }

    /// Admin s'est arrêté (fin de l'entrée standard) : les écrans du Mac reviennent.
    func shutdown() {
        _ = restore()
        clearPanels()
    }

    // MARK: veille et modes

    private func state(_ req: [String: Any]) throws -> [String: Any] {
        let ids = (req["ids"] as? [Any] ?? []).compactMap { ($0 as? NSNumber).map { CGDirectDisplayID($0.uint32Value) } }
        let active = activeDisplays()
        let modes: [[String: Int]] = ids.map { id in
            guard active.contains(id), let m = CGDisplayCopyDisplayMode(id) else { return ["width": 0, "height": 0, "refresh": 0] }
            let (w, h) = panel(id) ?? (m.pixelWidth, m.pixelHeight)
            return ["width": w, "height": h, "refresh": Int(m.refreshRate.rounded())]
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
        let hdr = modes[id]?.hdr ?? false
        let allScales = scaled.contains(id)
        guard display.apply(settings(width: w, height: h, refresh: hz, hidpi: hidpi, hdr: hdr, allScales: allScales)) else {
            throw Failure(message: "macOS refuse le mode \(w) × \(h) à \(hz) Hz")
        }
        modes[id] = (w, h, hz, hdr)
        if allScales {
            // L'échelle reste celle de macOS ; l'image suit la nouvelle définition.
            waitUntil(seconds: 3) { CGDisplayCopyDisplayMode(id) != nil }
            writePanels()
            let m = CGDisplayCopyDisplayMode(id)
            return ["width": w, "height": h, "refresh": Int((m?.refreshRate ?? Double(hz)).rounded())]
        }
        waitUntil(seconds: 3) { matchingMode(id, width: w, height: h, hidpi: hidpi) != nil }
        if let mode = matchingMode(id, width: w, height: h, hidpi: hidpi) {
            configure { c in CGConfigureDisplayWithDisplayMode(c, id, mode, nil) }
        }
        waitUntil(seconds: 3) { CGDisplayCopyDisplayMode(id)?.pixelWidth == w }
        guard let m = CGDisplayCopyDisplayMode(id) else { return ["width": 0, "height": 0, "refresh": 0] }
        return ["width": m.pixelWidth, "height": m.pixelHeight, "refresh": Int(m.refreshRate.rounded())]
    }

    // MARK: HDR

    /// Passe un écran virtuel en HDR ou en SDR (même mode, autre fonction de
    /// transfert) et attend que macOS le confirme (2 s au plus).
    private func setHDR(_ req: [String: Any]) throws -> [String: Any] {
        let id = CGDirectDisplayID(try int(req, "id"))
        let on = req["on"] as? Bool ?? false
        guard let display = virtuals.values.first(where: { $0.displayID == id }), let m = modes[id] else {
            return ["supported": false, "before": false, "active": false]
        }
        let before = hdrActive(id)
        if before != on {
            let hidpi = retina[id] ?? false
            let allScales = scaled.contains(id)
            let kept = CGDisplayCopyDisplayMode(id)
            guard display.apply(settings(width: m.w, height: m.h, refresh: m.hz, hidpi: hidpi, hdr: on, allScales: allScales)) else {
                throw Failure(message: "macOS refuse le HDR de l'écran virtuel")
            }
            modes[id] = (m.w, m.h, m.hz, on)
            waitUntil(seconds: 2) { hdrActive(id) == on }
            // Le changement de mode peut faire changer l'échelle : celle d'avant revient.
            if allScales, let k = kept, let mode = sameMode(id, as: k) {
                configure { c in CGConfigureDisplayWithDisplayMode(c, id, mode, nil) }
            } else if !allScales, hidpi, let mode = retinaMode(id, width: m.w, height: m.h) {
                configure { c in CGConfigureDisplayWithDisplayMode(c, id, mode, nil) }
            }
        }
        return ["supported": true, "before": before, "active": hdrActive(id)]
    }

    // MARK: échelles

    /// Définition de l'appareil d'un écran à toutes les échelles.
    private func panel(_ id: CGDirectDisplayID) -> (Int, Int)? {
        guard scaled.contains(id), let m = modes[id] else { return nil }
        return (m.w, m.h)
    }

    /// Pixels de l'appareil par point du bureau (3840 / 2560 = 1,5).
    private func scaleOf(_ id: CGDirectDisplayID) -> Double {
        let b = CGDisplayBounds(id)
        guard let (w, _) = panel(id), b.width > 0 else { return 2 }
        return Double(w) / b.width
    }

    /// Pour Kyber (correctif Mac 0014) : la définition de l'appareil de chaque
    /// écran à toutes les échelles, dans `virtual-displays.json`.
    private func writePanels() {
        let live = Set(virtuals.values.map(\.displayID))
        var out: [String: [String: Int]] = [:]
        for id in scaled where live.contains(id) {
            if let (w, h) = panel(id) { out[String(id)] = ["width": w, "height": h] }
        }
        writePanelFile(out)
    }

    func clearPanels() {
        writePanelFile([:])
    }

    // MARK: écrans du Mac

    /// Écrans du Mac pour le partage, à la définition de leur dalle : un écran
    /// 4K en mode « agrandi » (bureau de 3008 × 1692 points) est dessiné par
    /// macOS sur une image de 6016 × 3384, puis réduit ; on le présente comme
    /// l'écran 4K qu'il est (mode natif). Place à la même échelle.
    private func physicalDisplays() -> [[String: Any]] {
        activeDisplays().filter { !isVirtual($0) }.map { id in
            let b = CGDisplayBounds(id)
            let m = CGDisplayCopyDisplayMode(id)
            let (w, h) = nativeSize(id) ?? (m?.pixelWidth ?? Int(b.width), m?.pixelHeight ?? Int(b.height))
            let scale = b.width > 0 ? Double(w) / b.width : 1
            return ["id": id, "x": Int((b.origin.x * scale).rounded()), "y": Int((b.origin.y * scale).rounded()), "width": w,
                    "height": h, "refresh": Int((m?.refreshRate ?? 60).rounded()),
                    "primary": CGDisplayIsMain(id) != 0]
        }
    }

    /// Définition de la dalle : le mode que macOS marque natif
    /// (`kDisplayModeNativeFlag`), sinon le plus grand mode en 1x.
    private func nativeSize(_ id: CGDirectDisplayID) -> (Int, Int)? {
        let options = [kCGDisplayShowDuplicateLowResolutionModes: true] as CFDictionary
        guard let modes = CGDisplayCopyAllDisplayModes(id, options) as? [CGDisplayMode], !modes.isEmpty else { return nil }
        let nativeFlag: UInt32 = 0x0200_0000
        if let native = modes.first(where: { $0.ioFlags & nativeFlag != 0 }) {
            return (native.pixelWidth, native.pixelHeight)
        }
        return modes.filter { $0.pixelWidth == $0.width }.max { $0.pixelWidth * $0.pixelHeight < $1.pixelWidth * $1.pixelHeight }
            .map { ($0.pixelWidth, $0.pixelHeight) }
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
func settings(width w: Int, height h: Int, refresh hz: Int, hidpi: Bool, hdr: Bool = false, allScales: Bool = false) -> CGVirtualDisplaySettings {
    let s = CGVirtualDisplaySettings()
    s.hiDPI = hidpi ? 1 : 0
    // HDR : fonction de transfert 1 (voir CGVirtualDisplayPrivate.h).
    let sizes = allScales ? scaleList(width: w, height: h) : [hidpi ? (w / 2, h / 2) : (w, h)]
    s.modes = sizes.map { CGVirtualDisplayMode(width: UInt32($0.0), height: UInt32($0.1), refreshRate: Double(hz), transferFunction: hdr ? 1 : 0) }
    return s
}

/// À partir de cette largeur (4K), l'écran virtuel a toutes les échelles de
/// macOS, comme un vrai écran 4K ; en dessous (téléphones, tablettes), le
/// mode Retina seul, qui leur va mieux que la recommandation de macOS.
let scaledFromWidth = 3840

/// Échelles d'un écran de `width` × `height` pixels, en points (Retina : le
/// double en pixels) : du plus grand texte au plus d'espace, comme macOS les
/// propose pour un écran 4K (1280, 1600, 1920, 2400, 2560, 3008, 3360, 3840).
func scaleList(width w: Int, height h: Int) -> [(Int, Int)] {
    var out: [(Int, Int)] = []
    for f in [1.0 / 3, 5.0 / 12, 0.5, 0.625, 2.0 / 3, 47.0 / 60, 0.875, 1.0] {
        let pw = Int((Double(w) * f / 2).rounded()) * 2, ph = Int((Double(h) * f / 2).rounded()) * 2
        if !out.contains(where: { $0.0 == pw }) { out.append((pw, ph)) }
    }
    return out
}

/// Numéro de série d'un écran à toutes les échelles : sa place et sa
/// définition (macOS retient l'échelle choisie par numéro de série).
func scaledSerial(slot: Int, width w: Int, height h: Int) -> UInt32 {
    0x8000_0000 | UInt32(slot & 7) << 26 | UInt32((w / 2) & 0x1FFF) << 13 | UInt32((h / 2) & 0x1FFF)
}

/// Le mode de l'écran pareil à `m` (même bureau, mêmes pixels).
func sameMode(_ id: CGDirectDisplayID, as m: CGDisplayMode) -> CGDisplayMode? {
    let options = [kCGDisplayShowDuplicateLowResolutionModes: kCFBooleanTrue] as CFDictionary
    let modes = CGDisplayCopyAllDisplayModes(id, options) as? [CGDisplayMode] ?? []
    return modes.first { $0.isUsableForDesktopGUI() && $0.width == m.width && $0.height == m.height && $0.pixelWidth == m.pixelWidth }
}

/// `~/Library/Application Support/KyberNow/virtual-displays.json`, que lit Kyber.
func writePanelFile(_ panels: [String: [String: Int]]) {
    let dir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/KyberNow")
    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    guard let data = try? JSONSerialization.data(withJSONObject: panels, options: [.sortedKeys]) else { return }
    try? data.write(to: dir.appendingPathComponent("virtual-displays.json"), options: .atomic)
}

/// L'écran est-il en HDR ? (`SLSDisplayIsHDRModeEnabled`, privée.)
func hdrActive(_ id: CGDirectDisplayID) -> Bool {
    typealias Fn = @convention(c) (UInt32) -> Bool
    guard let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "SLSDisplayIsHDRModeEnabled") else { return false }
    return unsafeBitCast(sym, to: Fn.self)(id)
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

/// Allume ou éteint un écran du Mac (`CGSConfigureDisplayEnabled`, privé),
/// seul dans sa configuration, et attend que macOS l'ait fait (4 s au plus).
func setDisplayEnabled(_ id: CGDirectDisplayID, _ on: Bool) {
    typealias Fn = @convention(c) (CGDisplayConfigRef?, CGDirectDisplayID, Bool) -> CGError
    guard let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGSConfigureDisplayEnabled") else { return }
    let enable = unsafeBitCast(sym, to: Fn.self)
    var config: CGDisplayConfigRef?
    CGBeginDisplayConfiguration(&config)
    _ = enable(config, id, on)
    CGCompleteDisplayConfiguration(config, .forSession)
    waitUntil(seconds: 4) { activeDisplays().contains(id) == on }
}

/// Écrans du Mac éteints pendant la session et leur place d'avant, aussi
/// écrits sur disque pour les rallumer si l'agent s'arrête (voir l'en-tête).
struct OffState {
    var ids: [CGDirectDisplayID]
    var origins: [CGDirectDisplayID: CGPoint]

    static let file = URL(fileURLWithPath: NSHomeDirectory() + "/Library/Application Support/KyberNow/display-off.json")

    func save() {
        let body: [String: Any] = [
            "ids": ids,
            "origins": Dictionary(uniqueKeysWithValues: origins.map { ("\($0.key)", [Int($0.value.x), Int($0.value.y)]) }),
        ]
        try? FileManager.default.createDirectory(at: Self.file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? JSONSerialization.data(withJSONObject: body).write(to: Self.file)
    }

    static func load() -> OffState? {
        guard let data = try? Data(contentsOf: file),
              let body = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        let ids = (body["ids"] as? [NSNumber] ?? []).map { CGDirectDisplayID($0.uint32Value) }
        var origins: [CGDirectDisplayID: CGPoint] = [:]
        for (k, v) in body["origins"] as? [String: [Int]] ?? [:] where v.count == 2 {
            if let id = UInt32(k) { origins[id] = CGPoint(x: v[0], y: v[1]) }
        }
        return OffState(ids: ids, origins: origins)
    }

    /// Rallume les écrans un par un (le dernier éteint d'abord), remet leur
    /// place, puis efface le fichier.
    func switchOn() {
        for id in ids.reversed() { setDisplayEnabled(id, true) }
        var config: CGDisplayConfigRef?
        CGBeginDisplayConfiguration(&config)
        for (id, o) in origins where onlineDisplays().contains(id) {
            CGConfigureDisplayOrigin(config, id, Int32(o.x), Int32(o.y))
        }
        CGCompleteDisplayConfiguration(config, .forSession)
        try? FileManager.default.removeItem(at: Self.file)
    }
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

// Commandes ponctuelles du son (audio.swift) et des apps (apps.swift) : une
// réponse, puis la fin.
if CommandLine.arguments.count > 1, ["audio", "apps", "launch", "close", "locked"].contains(CommandLine.arguments[1]) {
    let command = CommandLine.arguments[1]
    let rest = Array(CommandLine.arguments.dropFirst(2))
    do {
        var body = try command == "audio" ? audioCommand(rest) : (appsCommand(command, rest) ?? [:])
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

// Écrans laissés éteints par un agent arrêté en route : rallumés d'abord.
// `kybernow-display recover` ne fait que cela (Admin le lance à son démarrage).
if let left = OffState.load() {
    left.switchOn()
}
if CommandLine.arguments.count > 1, CommandLine.arguments[1] == "recover" {
    reply(["ok": true])
    exit(0)
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
// Admin s'est arrêté : les écrans du Mac reviennent, macOS retire les écrans virtuels.
agent.shutdown()
exit(0)
