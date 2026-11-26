// L'extension caméra de KyberNow Station (docs/webcam.md, « Station Mac ») :
// à chaque lancement, l'app demande à macOS de l'activer (ou de la remplacer
// par celle de cette version). La première fois, macOS attend l'accord de la
// personne : une notification dit où le donner (Réglages Système › Général ›
// Ouverture et extensions › Extensions de caméra). Sans l'extension dans l'app
// (version construite sans le profil qui autorise « System Extension »), rien.

import Foundation
import SystemExtensions
import UserNotifications

final class CameraExtension: NSObject, OSSystemExtensionRequestDelegate {
    static let shared = CameraExtension()
    static let identifier = "fr.leslignesclaires.kybernow.station.camera"

    private var bundled: Bool {
        let url = Bundle.main.bundleURL
            .appendingPathComponent("Contents/Library/SystemExtensions/\(Self.identifier).systemextension")
        return FileManager.default.fileExists(atPath: url.path)
    }

    func activate() {
        guard bundled else { return }
        let request = OSSystemExtensionRequest.activationRequest(forExtensionWithIdentifier: Self.identifier, queue: .main)
        request.delegate = self
        OSSystemExtensionManager.shared.submitRequest(request)
    }

    func request(_ request: OSSystemExtensionRequest,
                 actionForReplacingExtension existing: OSSystemExtensionProperties,
                 withExtension ext: OSSystemExtensionProperties) -> OSSystemExtensionRequest.ReplacementAction {
        // Toujours celle de cette version (une app mise à jour, ou revenue à une
        // version d'avant, remplace l'autre).
        existing.bundleVersion == ext.bundleVersion && existing.bundleShortVersion == ext.bundleShortVersion ? .cancel : .replace
    }

    func requestNeedsUserApproval(_ request: OSSystemExtensionRequest) {
        let content = UNMutableNotificationContent()
        content.title = t("notify.camera")
        content.body = t("notify.cameraText")
        UNUserNotificationCenter.current().add(
            UNNotificationRequest(identifier: "camera-extension", content: content, trigger: nil))
    }

    func request(_ request: OSSystemExtensionRequest, didFinishWithResult result: OSSystemExtensionRequest.Result) {
        NSLog("KyberNow Station : extension caméra active (\(result == .completed ? "prête" : "au prochain démarrage"))")
        UNUserNotificationCenter.current().removeDeliveredNotifications(withIdentifiers: ["camera-extension"])
    }

    func request(_ request: OSSystemExtensionRequest, didFailWithError error: Error) {
        // `cancel` ci-dessus (même version) arrive ici aussi : rien à dire.
        let code = (error as NSError).code
        if code != OSSystemExtensionError.requestCanceled.rawValue {
            NSLog("KyberNow Station : extension caméra refusée (\(error.localizedDescription))")
        }
    }
}
