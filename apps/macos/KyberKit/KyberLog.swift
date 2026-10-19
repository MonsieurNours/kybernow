// Journaux du SDK Kyber vers le journal unifié de macOS (Console.app,
// sous-système fr.leslignesclaires.kybernow).

import os
@_implementationOnly import CKyber

public enum KyberLog {
    public static let app = Logger(subsystem: "fr.leslignesclaires.kybernow", category: "app")
    static let sdk = Logger(subsystem: "fr.leslignesclaires.kybernow", category: "kyber")

    static func install() {
        kyclient_set_log_cb({ level, target, line, _ in
            let text = "\(target.map { String(cString: $0) } ?? ""): \(line.map { String(cString: $0) } ?? "")"
            switch level {
            case KYCLIENT_LOG_LEVEL_ERROR: KyberLog.sdk.error("\(text, privacy: .public)")
            case KYCLIENT_LOG_LEVEL_WARN: KyberLog.sdk.warning("\(text, privacy: .public)")
            case KYCLIENT_LOG_LEVEL_INFO: KyberLog.sdk.info("\(text, privacy: .public)")
            default: KyberLog.sdk.debug("\(text, privacy: .public)")
            }
        }, KYCLIENT_LOG_LEVEL_FILTER_INFO, nil)
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
