// HDR (docs/hdr.md) : l'écran de l'appareil est-il HDR, ce que la Station
// propose, et la puce « HDR » de l'accueil.
//
// Écran HDR : marge EDR possible au-dessus de 1 sur Mac
// (`maximumPotentialExtendedDynamicRangeColorComponentValue`, pour chaque
// écran) et sur iPhone et iPad (`potentialEDRHeadroom`) ; sur l'Apple TV,
// téléviseur qui accepte le HDR10 (`AVPlayer.eligibleForHDRPlayback`).
// Chaque plateforme prévient de ses changements (fenêtre glissée sur un
// autre écran, écran branché, téléviseur qui coupe le HDR) : `HDRDisplay.observe`.
//
// En Debug, `-hdrScreen YES` (ou `NO`) impose la réponse, pour montrer la
// puce au simulateur ou sur un écran SDR.

import AVFoundation
import Foundation
@_implementationOnly import CKyberNow
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public enum HDRDisplay {
    /// Réponse imposée (`-hdrScreen YES|NO`, Debug seulement), sinon nil.
    public static var forced: Bool? {
        #if DEBUG
        guard UserDefaults.standard.object(forKey: "hdrScreen") != nil else { return nil }
        return UserDefaults.standard.bool(forKey: "hdrScreen")
        #else
        return nil
        #endif
    }

    #if os(macOS)
    /// Cet écran sait afficher le HDR (marge EDR possible au-dessus de 1).
    public static func isHDR(_ screen: NSScreen?) -> Bool {
        if let forced { return forced }
        guard let screen else { return false }
        return screen.maximumPotentialExtendedDynamicRangeColorComponentValue > 1
    }

    /// Marge EDR possible de l'écran (1 : SDR), pour le journal et la démonstration.
    public static func headroom(_ screen: NSScreen?) -> Double {
        Double(screen?.maximumPotentialExtendedDynamicRangeColorComponentValue ?? 1)
    }

    /// Les écrans changent (branchés, débranchés, réglages) : `handler` sur le fil principal.
    /// Une fenêtre qui passe sur un autre écran se suit par `NSWindow.didChangeScreenNotification`.
    public static func observe(_ handler: @escaping () -> Void) -> [NSObjectProtocol] {
        [NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { _ in handler() }]
    }
    #elseif os(iOS)
    public static func isHDR(_ screen: UIScreen?) -> Bool {
        if let forced { return forced }
        return (screen ?? UIScreen.main).potentialEDRHeadroom > 1
    }

    public static func headroom(_ screen: UIScreen?) -> Double {
        Double((screen ?? UIScreen.main).potentialEDRHeadroom)
    }

    /// Écran branché ou débranché (iPad), appareil qui revient au premier plan.
    public static func observe(_ handler: @escaping () -> Void) -> [NSObjectProtocol] {
        let c = NotificationCenter.default
        return [UIScreen.didConnectNotification, UIScreen.didDisconnectNotification, UIApplication.didBecomeActiveNotification].map {
            c.addObserver(forName: $0, object: nil, queue: .main) { _ in handler() }
        }
    }
    #else
    /// Le téléviseur accepte le HDR10 (tvOS le dit selon sa sortie et
    /// le réglage « Adapter la plage dynamique »).
    public static func isHDR(_ screen: UIScreen? = nil) -> Bool {
        if let forced { return forced }
        return AVPlayer.eligibleForHDRPlayback
    }

    /// tvOS ne donne pas de marge EDR : 1 (le téléviseur dit seulement s'il accepte le HDR10).
    public static func headroom(_ screen: UIScreen? = nil) -> Double { 1 }

    /// Le téléviseur gagne ou perd le HDR (sortie changée, téléviseur éteint).
    public static func observe(_ handler: @escaping () -> Void) -> [NSObjectProtocol] {
        let c = NotificationCenter.default
        return [AVPlayer.eligibleForHDRPlaybackDidChangeNotification, UIApplication.didBecomeActiveNotification].map {
            c.addObserver(forName: $0, object: nil, queue: .main) { _ in handler() }
        }
    }
    #endif
}

/// Ce que la Station dit du HDR (`hdr` de `GET /api/stream/info`).
public struct HDROffer: Sendable, Equatable {
    public enum Reason: String, Sendable {
        /// Station sans HDR, ou Admin réglé sur « Jamais ».
        case off
        /// Windows d'avant Windows 11 24H2.
        case windows
        /// Encodeur sans 10 bits.
        case encoder
        /// Sans écran virtuel, l'écran du PC n'est pas en HDR.
        case display
    }

    public let available: Bool
    public let reason: Reason?

    public init(available: Bool, reason: Reason? = nil) {
        self.available = available
        self.reason = available ? nil : (reason ?? .off)
    }

    /// `hdr` de l'info de la Station ; absent (Station d'avant) : pas de HDR.
    public init(json: Any?) {
        guard let j = json as? [String: Any] else {
            self.init(available: false, reason: .off)
            return
        }
        self.init(available: j["available"] as? Bool ?? false, reason: (j["reason"] as? String).flatMap(Reason.init(rawValue:)))
    }

    /// L'objet JSON tel que la Station l'envoie (pour le cœur).
    public var json: [String: Any] {
        var j: [String: Any] = ["available": available]
        if let reason { j["reason"] = reason.rawValue }
        return j
    }

    public static let none = HDROffer(available: false, reason: .off)
}

/// La puce « HDR » de l'accueil (et l'interrupteur de Réglages › Image) :
/// cachée, active, ou inactive avec la raison (docs/link-ui.md, section HDR).
public struct HDRState: Sendable, Equatable {
    /// Écran de l'appareil HDR : la puce existe (ailleurs, aucune trace).
    public var shown: Bool
    /// Le HDR est possible : la puce s'allume et s'éteint.
    public var available: Bool
    /// Inactive : la première raison qui s'applique, traduite.
    public var reason: String?

    public init(shown: Bool, available: Bool, reason: String?) {
        self.shown = shown
        self.available = available
        self.reason = reason
    }

    public static let hidden = HDRState(shown: false, available: false, reason: nil)

    /// Le HDR sera demandé : réglage allumé et HDR possible.
    public func on(_ setting: Bool) -> Bool { shown && available && setting }
}

public enum HDRSupport {
    /// La puce de l'accueil pour cet ordinateur : la règle du cœur
    /// (`kn_hdr_state`, `profile::hdr_state`), raisons dans l'ordre du tableau
    /// « Quand le HDR n'est pas possible » et dans la langue de l'app.
    /// `profile` : `jeu`, `bureau`, `max`, nil pour Personnalisé (un format
    /// H.264 choisi à la main bloque le HDR ; celui d'un profil passe à HEVC ou AV1).
    public static func state(offer: HDROffer?, profile: String? = nil, codec: VideoCodec, screenHDR: Bool,
                             capabilities: DecoderCapabilities = .current) -> HDRState {
        guard screenHDR else { return .hidden }
        var info: [String: Any] = [:]
        if let offer { info["hdr"] = offer.json }
        var settings: [String: Any] = ["codec": codec.rawValue, "mbps": 0]
        if let profile { settings["profile"] = profile }
        guard let infoText = text(info), let settingsText = text(settings), let capsText = text(SessionEngine.capabilitiesJSON(capabilities)),
              let out = kn_hdr_state(infoText, settingsText, capsText, screenHDR), let json = Core.take(out),
              let data = json.data(using: .utf8), let j = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        else { return .hidden }
        return HDRState(shown: j["shown"] as? Bool ?? false, available: j["available"] as? Bool ?? false,
                        reason: (j["reason"] as? String).flatMap { $0.isEmpty ? nil : $0 })
    }

    private static func text(_ object: [String: Any]) -> String? {
        (try? JSONSerialization.data(withJSONObject: object)).flatMap { String(data: $0, encoding: .utf8) }
    }
}

#if os(tvOS)
import AVKit
import CoreMedia

public extension HDRDisplay {
    /// Ce qu'on demande au téléviseur (`AVDisplayManager`) : la fréquence de
    /// la session (réglage « Adapter la fréquence » de tvOS) et, pour le HDR,
    /// une image HEVC PQ BT.2020 (réglage « Adapter la plage dynamique » :
    /// sans lui, le HDR suit la sortie choisie dans tvOS).
    static func criteria(refresh: Int, width: Int, height: Int, hdr: Bool) -> AVDisplayCriteria? {
        var extensions: [CFString: Any] = [:]
        if hdr {
            extensions[kCMFormatDescriptionExtension_ColorPrimaries] = kCMFormatDescriptionColorPrimaries_ITU_R_2020
            extensions[kCMFormatDescriptionExtension_TransferFunction] = kCMFormatDescriptionTransferFunction_SMPTE_ST_2084_PQ
            extensions[kCMFormatDescriptionExtension_YCbCrMatrix] = kCMFormatDescriptionYCbCrMatrix_ITU_R_2020
            extensions[kCMFormatDescriptionExtension_BitsPerComponent] = 10
        }
        var format: CMVideoFormatDescription?
        CMVideoFormatDescriptionCreate(allocator: nil, codecType: kCMVideoCodecType_HEVC, width: Int32(width), height: Int32(height),
                                       extensions: extensions.isEmpty ? nil : extensions as CFDictionary, formatDescriptionOut: &format)
        guard let format else { return nil }
        return AVDisplayCriteria(refreshRate: Float(refresh), formatDescription: format)
    }
}
#endif
