// Les trois autorisations de macOS, lues et demandées depuis le paquet de
// KyberNow Station (macOS les rattache au paquet de l'exécutable).
//
//   KyberNow Station --permissions check --out <fichier>
//   KyberNow Station --permissions request screen|accessibility|microphone
//
// L'installateur lance ces commandes en se désignant non responsable
// (`responsibility_spawnattrs_setdisclaim`) : c'est bien l'app installée que
// macOS regarde, pas l'installateur.

import AppKit
import ApplicationServices
import AVFoundation
import CoreGraphics

struct PermissionState {
    var screen: Bool
    var accessibility: Bool
    /// granted, denied ou undetermined.
    var microphone: String

    var count: Int { [screen, accessibility, microphone == "granted"].filter { $0 }.count }

    static func current() -> PermissionState {
        let mic: String
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: mic = "granted"
        case .denied, .restricted: mic = "denied"
        default: mic = "undetermined"
        }
        return PermissionState(screen: CGPreflightScreenCaptureAccess(), accessibility: AXIsProcessTrusted(), microphone: mic)
    }
}

/// Réglage de Confidentialité et sécurité de macOS.
func openPrivacyPane(_ anchor: String) {
    if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?\(anchor)") {
        NSWorkspace.shared.open(url)
    }
}

/// Mode `--permissions` : rend le code de sortie.
func permissionsCommand(_ args: [String]) -> Int32 {
    guard let action = args.first else { return 2 }
    switch action {
    case "check":
        let state = PermissionState.current()
        let json = "{\"screen\":\(state.screen),\"accessibility\":\(state.accessibility),\"microphone\":\"\(state.microphone)\"}"
        if let i = args.firstIndex(of: "--out"), i + 1 < args.count {
            do { try json.write(toFile: args[i + 1], atomically: true, encoding: .utf8) } catch { return 1 }
        } else {
            print(json)
        }
        return 0
    case "request":
        guard args.count > 1 else { return 2 }
        switch args[1] {
        case "screen":
            // Inscrit l'app dans la liste (la fenêtre de macOS la première fois), puis le réglage.
            if !CGPreflightScreenCaptureAccess() {
                _ = CGRequestScreenCaptureAccess()
                openPrivacyPane("Privacy_ScreenCapture")
            }
        case "accessibility":
            let key = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
            if !AXIsProcessTrustedWithOptions([key: true] as CFDictionary) {
                openPrivacyPane("Privacy_Accessibility")
            }
        case "microphone":
            switch AVCaptureDevice.authorizationStatus(for: .audio) {
            case .notDetermined:
                // La fenêtre de macOS reste ouverte tant que ce processus attend la réponse.
                let done = DispatchSemaphore(value: 0)
                AVCaptureDevice.requestAccess(for: .audio) { _ in done.signal() }
                _ = done.wait(timeout: .now() + 600)
            case .authorized: break
            default: openPrivacyPane("Privacy_Microphone")
            }
        default:
            return 2
        }
        return 0
    default:
        return 2
    }
}
