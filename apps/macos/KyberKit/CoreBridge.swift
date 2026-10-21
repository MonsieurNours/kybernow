// Le cœur Rust (apps/core, par le pont C apps/apple) vu de Swift : journal,
// langue, et les petits outils communs à la console et au moteur de session.
// Mac et Apple TV (docs/coeur-apple.md).

import Foundation
import os
@_implementationOnly import CKyberNow

public enum Core {
    /// Branche le journal du cœur sur le journal unifié (catégorie `core`)
    /// et lui donne la langue de l'app. À appeler une fois au lancement.
    public static func install() {
        _ = installed
    }

    private static let installed: Void = {
        KyberLog.install()
        kn_set_log_callback { level, target, message in
            let text = "\(target.map { String(cString: $0) } ?? ""): \(message.map { String(cString: $0) } ?? "")"
            switch level {
            case 1: KyberLog.core.error("\(text, privacy: .public)")
            case 2: KyberLog.core.warning("\(text, privacy: .public)")
            case 3: KyberLog.core.notice("\(text, privacy: .public)")
            default: KyberLog.core.debug("\(text, privacy: .public)")
            }
        }
        kn_set_language(LanguageChoice.effective)
    }()

    /// Langue choisie dans les réglages (`fr`, `en`) : messages du cœur et
    /// `Accept-Language` vers la console.
    public static func setLanguage(_ tag: String) {
        kn_set_language(tag)
    }

    /// Maintenant, sur l'horloge des mesures de Kyber (µs).
    static func nowMicros() -> Int64 { kn_now_micros() }

    /// Chaîne rendue par le cœur, libérée après lecture.
    static func take(_ p: UnsafeMutablePointer<CChar>?) -> String? {
        guard let p else { return nil }
        defer { kn_string_free(p) }
        return String(cString: p)
    }
}

extension KyberLog {
    static let core = Logger(subsystem: "fr.leslignesclaires.kybernow", category: "core")
}
