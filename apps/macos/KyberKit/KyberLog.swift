// Journaux vers le journal unifié (Console.app, sous-système
// fr.leslignesclaires.kybernow) : l'app, l'image, le son, et libkynput
// (catégorie kyber). Le cœur Rust a sa catégorie, core (CoreBridge.swift).

import os
@_implementationOnly import CKyber

public enum KyberLog {
    public static let app = Logger(subsystem: "fr.leslignesclaires.kybernow", category: "app")
    static let sdk = Logger(subsystem: "fr.leslignesclaires.kybernow", category: "kyber")
    static let video = Logger(subsystem: "fr.leslignesclaires.kybernow", category: "video")
    static let audio = Logger(subsystem: "fr.leslignesclaires.kybernow", category: "audio")

    /// Branche le journal de libkynput (une fois, par `Core.install`).
    static func install() {
        kynput_set_log_cb({ level, target, line, _ in
            let text = "\(target.map { String(cString: $0) } ?? ""): \(line.map { String(cString: $0) } ?? "")"
            switch level {
            case KYNPUT_LOG_LEVEL_ERROR: KyberLog.sdk.error("\(text, privacy: .public)")
            case KYNPUT_LOG_LEVEL_WARN: KyberLog.sdk.warning("\(text, privacy: .public)")
            default: KyberLog.sdk.debug("\(text, privacy: .public)")
            }
        }, KYNPUT_LOG_LEVEL_FILTER_WARN, nil)
    }
}
