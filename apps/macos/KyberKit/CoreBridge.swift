// Le cœur Rust (apps/core, par le pont C apps/apple) vu de Swift : journal,
// langue, et les petits outils communs à la console et au moteur de session.
// Mac, Apple TV, iPhone et iPad (docs/coeur-apple.md).

import Foundation
import os
@_implementationOnly import CKyberNow

public enum Core {
    /// Manettes que voit le lecteur du cœur (une partie sur console ignore le
    /// clavier dès qu'il y en a une) ; nil tant que son lecteur ne tourne pas.
    public static var gamepadCount: Int? {
        let n = kn_gamepads_count()
        return n < 0 ? nil : Int(n)
    }

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
        // Mac, iPhone et iPad ont un micro (docs/micro.md) ; jamais l'Apple TV.
        #if os(macOS) || os(iOS)
        kn_set_device_has_microphone(true)
        #endif
        // Les fins de l'app que le système rapporte, pour « Envoyer le journal ».
        AppJournal.watchExits()
    }()

    /// Chaîne rendue par le cœur, libérée après lecture.
    static func take(_ p: UnsafeMutablePointer<CChar>?) -> String? {
        guard let p else { return nil }
        defer { kn_string_free(p) }
        return String(cString: p)
    }
}

extension TargetBitrate {
    /// La ligne « Débit visé » dans la langue de l'app : « 42 / 60 Mbit/s ·
    /// baissé, réseau saturé », « 60 Mbit/s · Auto »… (`kn_target_bitrate_text`).
    public var text: String {
        Core.take(kn_target_bitrate_text(bitrate, profile, reason)) ?? ""
    }

    /// Le libellé de la ligne : « Débit visé » / “Target bitrate”.
    public static var label: String {
        Core.take(kn_target_bitrate_label()) ?? ""
    }
}

extension KyberLog {
    static let core = Logger(subsystem: "fr.leslignesclaires.kybernow", category: "core")
}
