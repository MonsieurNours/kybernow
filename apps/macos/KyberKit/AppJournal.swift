// « Envoyer le journal » (docs/link-ui.md, « Réglages de l'app » › À
// propos) : un fichier texte `kybernow-journal.txt` qui raconte ce qui s'est
// passé quand l'app se ferme ou qu'une session se coupe. En tête : l'app et
// sa version, l'appareil et son système, la langue, les fins précédentes de
// l'app que le système donne (plantage, « ne répond pas » : MetricKit, Mac,
// iPhone et iPad) ; puis le journal de l'app depuis son lancement (journal
// unifié, sous-système fr.leslignesclaires.kybernow : app, image, son,
// Kyber, cœur, paniques comprises). Rien ne part sans le geste de
// l'utilisateur ; ni mot de passe, ni jeton, ni code dans le fichier.
//
// Mac, iPhone, iPad : le fichier, à donner à la feuille de partage.
// Apple TV : dépôt chiffré sur le relais et QR code (pont
// apps/apple/src/journal.rs, docs/relais.md, « Dépôt d'un journal »).

import Foundation
import OSLog
#if canImport(MetricKit)
import MetricKit
#endif
#if canImport(UIKit)
import UIKit
#endif
@_implementationOnly import CKyberNow

public enum AppJournal {
    /// Le nom du fichier, toujours le même.
    public static let fileName = "kybernow-journal.txt"
    static let subsystem = "fr.leslignesclaires.kybernow"

    // MARK: - Contenu

    /// L'en-tête (app, appareil, système, langue, fins précédentes) et le
    /// journal de l'app depuis son lancement, du plus ancien au plus récent.
    public static func collect() async -> (header: String, body: String) {
        await Task.detached(priority: .userInitiated) {
            (header(), body())
        }.value
    }

    /// Mac, iPhone, iPad : le journal dans un fichier temporaire, à partager.
    public static func file() async throws -> URL {
        let (header, body) = await collect()
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("kybernow-journal", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let url = folder.appendingPathComponent(fileName)
        try Data((header + body).utf8).write(to: url, options: .atomic)
        return url
    }

    static func header() -> String {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? "?"
        let build = info?["CFBundleVersion"] as? String ?? "?"
        let os = ProcessInfo.processInfo.operatingSystemVersion
        // « Appareil : » et « Journal écrit le » : les deux lignes que la page
        // du téléphone lit dans les 4 premiers Kio (docs/relais.md, « Dépôt
        // d'un journal »), comme l'en-tête du journal d'Android.
        let stamp = DateFormatter()
        stamp.locale = Locale(identifier: "en_US_POSIX")
        stamp.timeZone = TimeZone(identifier: "UTC")
        stamp.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS 'UTC'"
        var lines = [
            "KyberNow Link \(version) (\(build))",
            "Appareil : Apple \(model())",
            "\(systemName()) \(os.majorVersion).\(os.minorVersion).\(os.patchVersion)",
            "Langue de l'app : \(LanguageChoice.effective)",
            "Journal écrit le \(stamp.string(from: Date()))",
        ]
        lines.append(contentsOf: previousExits())
        return lines.joined(separator: "\n") + "\n\n"
    }

    /// L'identifiant du modèle (« Mac15,3 », « iPhone17,2 », « AppleTV14,1 ») :
    /// jamais le nom de l'appareil, qui dit souvent le prénom de quelqu'un.
    static func model() -> String {
        #if os(macOS)
        var size = 0
        sysctlbyname("hw.model", nil, &size, nil, 0)
        var bytes = [CChar](repeating: 0, count: max(size, 1))
        sysctlbyname("hw.model", &bytes, &size, nil, 0)
        return String(cString: bytes)
        #else
        var u = utsname()
        uname(&u)
        return withUnsafeBytes(of: &u.machine) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
        #endif
    }

    static func systemName() -> String {
        #if os(macOS)
        "macOS"
        #elseif os(tvOS)
        "tvOS"
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "iPadOS" : "iOS"
        #endif
    }

    /// Les fins précédentes de l'app que le système a données (MetricKit, les
    /// dernières 24 h) : plantages et moments où l'app ne répondait plus.
    static func previousExits() -> [String] {
        #if canImport(MetricKit) && !os(tvOS)
        let payloads = MXMetricManager.shared.pastDiagnosticPayloads
        var lines: [String] = []
        let date = ISO8601DateFormatter()
        for p in payloads {
            let when = date.string(from: p.timeStampEnd)
            for crash in p.crashDiagnostics ?? [] {
                let type = crash.exceptionType.map { "exception \($0)" } ?? ""
                let signal = crash.signal.map { "signal \($0)" } ?? ""
                let reason = crash.terminationReason ?? ""
                let detail = [type, signal, reason].filter { !$0.isEmpty }.joined(separator: ", ")
                lines.append("Fin précédente : plantage (\(detail)), \(when), version \(crash.applicationVersion)")
            }
            for hang in p.hangDiagnostics ?? [] {
                lines.append("Fin précédente : ne répond pas (\(hang.hangDuration)), \(when), version \(hang.applicationVersion)")
            }
        }
        return lines.isEmpty ? ["Fins précédentes : aucune donnée par le système"] : lines
        #else
        return ["Fins précédentes : tvOS ne les donne pas"]
        #endif
    }

    /// À appeler au lancement (Mac, iPhone, iPad) : le système garde alors
    /// les rapports des fins de l'app pour `previousExits`.
    public static func watchExits() {
        #if canImport(MetricKit) && !os(tvOS)
        MXMetricManager.shared.add(ExitWatch.shared)
        #endif
    }

    /// Le journal unifié de l'app depuis son lancement.
    static func body() -> String {
        guard let store = try? OSLogStore(scope: .currentProcessIdentifier) else {
            return "(journal du système illisible)\n"
        }
        let start = store.position(timeIntervalSinceLatestBoot: 0)
        let predicate = NSPredicate(format: "subsystem == %@", subsystem)
        guard let entries = try? store.getEntries(at: start, matching: predicate) else {
            return "(journal du système illisible)\n"
        }
        let time = DateFormatter()
        time.locale = Locale(identifier: "en_US_POSIX")
        time.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS"
        var out = ""
        for case let e as OSLogEntryLog in entries {
            let level: String
            switch e.level {
            case .fault: level = "F"
            case .error: level = "E"
            case .notice: level = "N"
            case .info: level = "I"
            case .debug: level = "D"
            default: level = "-"
            }
            out += "\(time.string(from: e.date)) \(level) [\(e.category)] \(e.composedMessage)\n"
        }
        return out.isEmpty ? "(journal vide)\n" : out
    }

    // MARK: - Dépôt par QR code (Apple TV)

    /// Un dépôt sur le relais : l'adresse du QR code (clé comprise), et de
    /// quoi le suivre et le retirer.
    public struct Deposit: Sendable, Equatable {
        public let id: String
        let owner: String
        public let url: String
        public let expires: Date
        let relay: String
    }

    /// Pourquoi le dépôt n'a pas pu se faire.
    public enum Failure: Error, Sendable, Equatable {
        /// « Le relais ne répond pas : vérifiez la connexion à Internet, puis réessayez. »
        case unreachable
        /// « Ce relais ne prend pas encore les journaux : il doit être mis à jour. »
        case oldRelay
        /// « Trop d'envois : réessayez dans une heure. »
        case tooMany
        case other(String)
    }

    /// Le journal chiffré sur cet appareil et déposé sur le relais (celui du
    /// compte KyberNow de l'appareil, sinon le relais public). Hors du fil
    /// principal, 30 s au plus.
    public static func upload(relay: String?) async -> Result<Deposit, Failure> {
        let (header, body) = await collect()
        let relay = relay ?? ""
        return await Task.detached(priority: .userInitiated) {
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            let code = kn_journal_upload(relay, header, body, &out, &error)
            let detail = Core.take(error) ?? ""
            guard code == KN_OK, let json = Core.take(out),
                  let v = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any],
                  let id = v["id"] as? String, let owner = v["owner"] as? String, let url = v["url"] as? String else {
                switch code {
                case Int32(KN_JOURNAL_UNREACHABLE): return .failure(.unreachable)
                case Int32(KN_JOURNAL_OLD_RELAY): return .failure(.oldRelay)
                case Int32(KN_JOURNAL_TOO_MANY): return .failure(.tooMany)
                default:
                    KyberLog.app.error("journal : dépôt impossible (\(detail, privacy: .public))")
                    return .failure(.other(detail))
                }
            }
            let seconds = (v["expires_in"] as? NSNumber)?.doubleValue ?? 3600
            return .success(Deposit(id: id, owner: owner, url: url, expires: Date().addingTimeInterval(seconds), relay: relay))
        }.value
    }

    /// Où en est un dépôt (l'écran du QR code le relit toutes les 2 s).
    public enum State: Sendable { case waiting, opened, gone, unreachable }

    public static func state(_ d: Deposit) async -> State {
        await Task.detached {
            switch kn_journal_state(d.relay, d.id, d.owner) {
            case Int32(KN_JOURNAL_OPENED): return .opened
            case Int32(KN_JOURNAL_GONE): return .gone
            case Int32(KN_JOURNAL_WAITING): return .waiting
            default: return .unreachable
            }
        }.value
    }

    /// « Terminé » : retire le dépôt s'il n'a pas été ouvert.
    public static func remove(_ d: Deposit) async {
        await Task.detached { _ = kn_journal_delete(d.relay, d.id, d.owner) }.value
    }
}

#if canImport(MetricKit) && !os(tvOS)
/// Abonné de MetricKit : rien à faire à la livraison, les rapports sont lus
/// à la préparation du journal (`pastDiagnosticPayloads`).
final class ExitWatch: NSObject, MXMetricManagerSubscriber {
    static let shared = ExitWatch()
}
#endif
