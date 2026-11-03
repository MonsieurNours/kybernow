// Apps du Mac pour l'« App au début de la session » (docs/station-macos.md,
// jalon 7), pour Admin (`launch.rs`, module `platform` du Mac) : commandes
// ponctuelles, une réponse JSON sur la sortie.
//
//   kybernow-display apps --icons <dossier>
//       → {"apps": [{"id": "<identifiant du paquet>", "name", "path", "icon": "<fichier>"}],
//          "steam": "<chemin de Steam.app>" | null, "user": "<compte>"}
//   kybernow-display launch --kind app|uri|path --target <…>
//       → {"front": vrai si l'app était déjà ouverte, "pid": n | null}
//   kybernow-display close [--uri <adresse>] [--pid n] [--exe <identifiant ou chemin>]
//       → {"closed": n}
//   kybernow-display locked → {"locked": bool}
//
// Les apps : celles des dossiers Applications (du Mac, d'Utilitaires, du
// système et de l'utilisateur), reconnues par leur identifiant de paquet ;
// icônes de 64 × 64 en PNG, écrites dans le dossier donné.

import AppKit
import Foundation

private let appFolders: [String] = [
    "/Applications", "/Applications/Utilities", "/System/Applications", "/System/Applications/Utilities",
    NSHomeDirectory() + "/Applications",
]

private func option(_ args: [String], _ name: String) -> String? {
    guard let i = args.firstIndex(of: name), i + 1 < args.count else { return nil }
    return args[i + 1]
}

private func pngIcon(for path: String, size: Int = 64) -> Data? {
    let image = NSWorkspace.shared.icon(forFile: path)
    guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
                                     samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                                     bytesPerRow: 0, bitsPerPixel: 0) else { return nil }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    image.draw(in: NSRect(x: 0, y: 0, width: size, height: size), from: .zero, operation: .copy, fraction: 1)
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])
}

private func listApps(icons: String) -> [String: Any] {
    try? FileManager.default.createDirectory(atPath: icons, withIntermediateDirectories: true)
    var seen = Set<String>()
    var apps: [[String: Any]] = []
    var steam: String?
    for folder in appFolders {
        guard let names = try? FileManager.default.contentsOfDirectory(atPath: folder) else { continue }
        for name in names.sorted() where name.hasSuffix(".app") {
            let path = folder + "/" + name
            guard let bundle = Bundle(path: path), let id = bundle.bundleIdentifier, !seen.contains(id) else { continue }
            seen.insert(id)
            if id == "com.valvesoftware.steam" { steam = path }
            let display = (bundle.localizedInfoDictionary?["CFBundleDisplayName"] as? String)
                ?? (bundle.infoDictionary?["CFBundleDisplayName"] as? String)
                ?? FileManager.default.displayName(atPath: path).replacingOccurrences(of: ".app", with: "")
            var entry: [String: Any] = ["id": id, "name": display, "path": path]
            let file = "\(apps.count).png"
            if let png = pngIcon(for: path), (try? png.write(to: URL(fileURLWithPath: icons + "/" + file))) != nil {
                entry["icon"] = file
            }
            apps.append(entry)
        }
    }
    return ["apps": apps, "steam": steam ?? NSNull(), "user": NSUserName()]
}

private func running(_ exe: String) -> [NSRunningApplication] {
    NSWorkspace.shared.runningApplications.filter { $0.bundleIdentifier == exe || $0.bundleURL?.path == exe }
}

private func openApp(_ url: URL) throws -> NSRunningApplication? {
    let done = DispatchSemaphore(value: 0)
    var app: NSRunningApplication?
    var failure: Error?
    let config = NSWorkspace.OpenConfiguration()
    config.activates = true
    NSWorkspace.shared.openApplication(at: url, configuration: config) { a, e in
        app = a
        failure = e
        done.signal()
    }
    if done.wait(timeout: .now() + 30) == .timedOut { throw Failure(message: "l'app ne s'est pas ouverte en 30 s") }
    if let failure { throw Failure(message: failure.localizedDescription) }
    return app
}

private func launch(_ args: [String]) throws -> [String: Any] {
    guard let target = option(args, "--target"), !target.isEmpty else { throw Failure(message: "--target manquant") }
    switch option(args, "--kind") ?? "app" {
    case "app":
        guard let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: target) else {
            throw Failure(message: "app introuvable : \(target)")
        }
        let front = !running(target).isEmpty
        let app = try openApp(url)
        return ["front": front, "pid": app.map { Int($0.processIdentifier) } ?? NSNull()]
    case "uri":
        guard let url = URL(string: target), NSWorkspace.shared.open(url) else { throw Failure(message: "adresse refusée : \(target)") }
        return ["front": false, "pid": NSNull()]
    default:
        let url = URL(fileURLWithPath: target)
        if target.hasSuffix(".app") {
            let front = !running(target).isEmpty
            let app = try openApp(url)
            return ["front": front, "pid": app.map { Int($0.processIdentifier) } ?? NSNull()]
        }
        guard NSWorkspace.shared.open(url) else { throw Failure(message: "fichier refusé : \(target)") }
        return ["front": false, "pid": NSNull()]
    }
}

private func close(_ args: [String]) throws -> [String: Any] {
    if let uri = option(args, "--uri"), let url = URL(string: uri) {
        return ["closed": NSWorkspace.shared.open(url) ? 1 : 0]
    }
    var apps: [NSRunningApplication] = []
    if let pid = option(args, "--pid").flatMap({ pid_t($0) }), let app = NSRunningApplication(processIdentifier: pid) {
        apps.append(app)
    } else if let exe = option(args, "--exe") {
        apps = running(exe)
    }
    return ["closed": apps.filter { $0.terminate() }.count]
}

/// Verrouille la session (fin d'un prêt) : `SACLockScreenImmediate`, la
/// fonction du menu Pomme › Verrouiller l'écran (login.framework, privée).
private func lockScreen() throws {
    guard let handle = dlopen("/System/Library/PrivateFrameworks/login.framework/Versions/Current/login", RTLD_NOW),
          let symbol = dlsym(handle, "SACLockScreenImmediate") else {
        throw Failure(message: "verrouillage indisponible (login.framework)")
    }
    typealias Lock = @convention(c) () -> Int32
    let status = unsafeBitCast(symbol, to: Lock.self)()
    if status != 0 { throw Failure(message: "verrouillage refusé (\(status))") }
}

private func screenLocked() -> Bool {
    guard let info = CGSessionCopyCurrentDictionary() as? [String: Any] else { return false }
    return info["CGSSessionScreenIsLocked"] as? Bool ?? false
}

/// Commandes ponctuelles des apps ; `nil` : pas une commande d'ici.
func appsCommand(_ command: String, _ args: [String]) throws -> [String: Any]? {
    switch command {
    case "apps":
        guard let icons = option(args, "--icons") else { throw Failure(message: "--icons manquant") }
        return listApps(icons: icons)
    case "launch": return try launch(args)
    case "close": return try close(args)
    case "locked": return ["locked": screenLocked()]
    case "lock":
        try lockScreen()
        return ["locked": true]
    default: return nil
    }
}
