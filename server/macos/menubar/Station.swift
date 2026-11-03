// Ce que l'icône sait de la Station : services launchd de la session,
// `kybernow.json`, et l'état que donne Admin (`GET /api/local/state`, avec
// le jeton local et le certificat d'Admin épinglé, comme l'icône Windows).

import CryptoKit
import Foundation

enum Labels {
    static let kyber = "fr.leslignesclaires.kybernow.station.kyber"
    static let admin = "fr.leslignesclaires.kybernow.station.admin"
    static let menubar = "fr.leslignesclaires.kybernow.station.menubar"
    static let all = [kyber, admin, menubar]
}

enum Paths {
    static let home = FileManager.default.homeDirectoryForCurrentUser
    static var station: URL {
        if let dir = ProcessInfo.processInfo.environment["KYBERNOW_STATION_DIR"], !dir.isEmpty { return URL(fileURLWithPath: dir) }
        return home.appendingPathComponent("Library/Application Support/KyberNow/Station")
    }
    static var admin: URL { station.appendingPathComponent("admin") }
    static var record: URL { station.appendingPathComponent("kybernow.json") }
    static func plist(_ label: String) -> URL { home.appendingPathComponent("Library/LaunchAgents/\(label).plist") }
    /// Exécutables de l'app : celle qui contient l'icône.
    static func exe(_ name: String) -> URL { Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/\(name)") }
}

// MARK: - launchd

@discardableResult
func run(_ program: String, _ args: [String]) -> (Int32, String) {
    let p = Process()
    p.executableURL = URL(fileURLWithPath: program)
    p.arguments = args
    let pipe = Pipe()
    p.standardOutput = pipe
    p.standardError = pipe
    do { try p.run() } catch { return (-1, "\(error)") }
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    p.waitUntilExit()
    return (p.terminationStatus, String(decoding: data, as: UTF8.self))
}

let domain = "gui/\(getuid())"

enum ServiceState { case running, stopped, starting, missing }

func serviceState(_ label: String) -> ServiceState {
    let (code, out) = run("/bin/launchctl", ["print", "\(domain)/\(label)"])
    if code != 0 { return FileManager.default.fileExists(atPath: Paths.plist(label).path) ? .stopped : .missing }
    let state = out.split(separator: "\n").map { $0.trimmingCharacters(in: .whitespaces) }.first { $0.hasPrefix("state = ") }?.dropFirst(8) ?? ""
    switch state {
    case "running": return .running
    case "spawn scheduled", "xpcproxy": return .starting
    default: return .stopped
    }
}

func startService(_ label: String) {
    if run("/bin/launchctl", ["print", "\(domain)/\(label)"]).0 != 0 {
        run("/bin/launchctl", ["bootstrap", domain, Paths.plist(label).path])
    }
    run("/bin/launchctl", ["kickstart", "\(domain)/\(label)"])
}

func stopService(_ label: String) {
    if run("/bin/launchctl", ["print", "\(domain)/\(label)"]).0 == 0 {
        run("/bin/launchctl", ["bootout", "\(domain)/\(label)"])
    }
}

func restartService(_ label: String) {
    if run("/bin/launchctl", ["print", "\(domain)/\(label)"]).0 == 0 {
        run("/bin/launchctl", ["kickstart", "-k", "\(domain)/\(label)"])
    } else {
        startService(label)
    }
}

/// « Démarrer avec le Mac » : `RunAtLoad` des trois services.
func startsWithMac() -> Bool {
    guard let data = try? Data(contentsOf: Paths.plist(Labels.kyber)),
          let plist = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any] else { return false }
    return plist["RunAtLoad"] as? Bool ?? false
}

func setStartsWithMac(_ on: Bool) {
    for label in Labels.all {
        let url = Paths.plist(label)
        guard let data = try? Data(contentsOf: url),
              var plist = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any] else { continue }
        plist["RunAtLoad"] = on
        if let out = try? PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0) {
            try? out.write(to: url, options: .atomic)
        }
    }
    // Noté aussi dans kybernow.json (réinstallation).
    if var record = readRecord() {
        record["start_with_mac"] = on
        if let out = try? JSONSerialization.data(withJSONObject: record, options: [.prettyPrinted, .sortedKeys]) {
            try? out.write(to: Paths.record, options: .atomic)
        }
    }
}

// MARK: - Admin

func readRecord() -> [String: Any]? {
    guard let data = try? Data(contentsOf: Paths.record) else { return nil }
    return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
}

var consolePort: Int? { readRecord()?["console_port"] as? Int }

func adminURL(_ page: String = "") -> URL? {
    guard let port = consolePort else { return nil }
    return URL(string: "https://localhost:\(port)/\(page.isEmpty ? "" : "#/\(page)")")
}

/// Client d'Admin : n'accepte que le certificat dont l'empreinte est notée
/// dans `kybernow.json` (`console_fingerprint`, « sha256:aa:bb:… »).
final class AdminClient: NSObject, URLSessionDelegate {
    private lazy var session = URLSession(configuration: .ephemeral, delegate: self, delegateQueue: nil)

    private func pinned() -> Data? {
        guard let text = readRecord()?["console_fingerprint"] as? String else { return nil }
        let hex = text.replacingOccurrences(of: "sha256:", with: "").replacingOccurrences(of: ":", with: "")
        var bytes = [UInt8]()
        var i = hex.startIndex
        while i < hex.endIndex, let j = hex.index(i, offsetBy: 2, limitedBy: hex.endIndex) {
            guard let b = UInt8(hex[i..<j], radix: 16) else { return nil }
            bytes.append(b)
            i = j
        }
        return bytes.count == 32 ? Data(bytes) : nil
    }

    func urlSession(_ session: URLSession, didReceive challenge: URLAuthenticationChallenge,
                    completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        guard challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust,
              let trust = challenge.protectionSpace.serverTrust,
              let chain = SecTrustCopyCertificateChain(trust) as? [SecCertificate], let leaf = chain.first,
              let expected = pinned() else {
            completionHandler(.cancelAuthenticationChallenge, nil)
            return
        }
        let der = SecCertificateCopyData(leaf) as Data
        if Data(SHA256.hash(data: der)) == expected {
            completionHandler(.useCredential, URLCredential(trust: trust))
        } else {
            completionHandler(.cancelAuthenticationChallenge, nil)
        }
    }

    private var token: String {
        (try? String(contentsOf: Paths.admin.appendingPathComponent("local-token"), encoding: .utf8))?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    }

    /// Appel bloquant (fil de fond) ; `nil` si Admin ne répond pas.
    func call(_ method: String, _ path: String, body: [String: Any]? = nil) -> [String: Any]? {
        guard let port = consolePort, let url = URL(string: "https://127.0.0.1:\(port)\(path)") else { return nil }
        var request = URLRequest(url: url, timeoutInterval: 4)
        request.httpMethod = method
        request.setValue(token, forHTTPHeaderField: "x-kyber-local")
        request.setValue(lang == .fr ? "fr" : "en", forHTTPHeaderField: "Accept-Language")
        if let body, let data = try? JSONSerialization.data(withJSONObject: body) {
            request.httpBody = data
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        let done = DispatchSemaphore(value: 0)
        var result: [String: Any]?
        session.dataTask(with: request) { data, response, _ in
            if let data, (response as? HTTPURLResponse)?.statusCode == 200 {
                result = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
            }
            done.signal()
        }.resume()
        _ = done.wait(timeout: .now() + 5)
        return result
    }
}

/// Instantané de la Station pour le menu.
struct Snapshot {
    var kyber: ServiceState = .missing
    var admin: ServiceState = .missing
    var state: [String: Any]?
    var startWithMac = false
    var permissions = PermissionState(screen: false, accessibility: false, microphone: "undetermined")

    var live: [String: Any]? { (state?["activity"] as? [String: Any])?["live"] as? [String: Any] }
    var last: [String: Any]? { (state?["activity"] as? [String: Any])?["last"] as? [String: Any] }
    var update: String? { state?["update_available"] as? String }
    var version: String { state?["version"] as? String ?? (readRecord()?["version"] as? String ?? "") }
    var requests: Int { state?["pair_requests_count"] as? Int ?? 0 }
    var sharing: Bool { state?["share"] is [String: Any] }
    var lend: [String: Any]? { state?["lend"] as? [String: Any] }
    var lent: Bool {
        guard let lend, lend["borrower"] is [String: Any] else { return false }
        return ["preparing", "lent", "ending"].contains(lend["state"] as? String ?? "")
    }
    var borrower: String { ((lend?["borrower"] as? [String: Any])?["name"] as? String) ?? "" }
    var lendUntil: Double? { (lend?["borrower"] as? [String: Any])?["until"] as? Double }
    /// Fin annoncée (préavis de reprise ou fin prévue), en secondes Unix.
    var lendNotice: Double? { (lend?["notice"] as? [String: Any])?["at"] as? Double }
    /// Demandes d'emprunt en attente (`id`, `name`).
    var lendRequests: [[String: Any]] { lend?["requests"] as? [[String: Any]] ?? [] }
    var inSession: Bool { live != nil || (state?["session"] as? Bool ?? false) }
}
