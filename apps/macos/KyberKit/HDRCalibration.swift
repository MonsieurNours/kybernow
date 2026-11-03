// Réglage du HDR (docs/hdr.md, « Réglage du HDR ») : les valeurs de chaque
// écran HDR de l'appareil, données au cœur avec la session, et l'écran de
// quatre mires qui les règle à l'œil.
//
// Valeurs automatiques : écrans Apple d'après leur marge EDR possible et leur
// modèle (XDR des MacBook Pro et du Pro Display, iPhone et iPad à écran XDR) ;
// sinon les valeurs par défaut du contrat (1 000 / 1 000 / 0 / 203 nits,
// l'Apple TV par exemple). Les valeurs réglées à l'œil sont gardées pour cet
// écran de l'appareil (Mac : vendeur, modèle et numéro de série de l'écran)
// dans les préférences de l'app, et remplacent les automatiques.

import Combine
import Foundation
@_implementationOnly import CKyberNow
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Les lumières d'un écran, en nits (`HdrLuminance` du cœur).
public struct HDRLuminance: Codable, Equatable, Sendable {
    /// Luminosité maximale, sur une petite zone.
    public var max: Double
    /// Luminosité sur tout l'écran.
    public var fullFrame: Double
    /// Noir.
    public var min: Double
    /// Blanc du bureau (niveau de blanc SDR de Windows).
    public var sdrWhite: Double

    public init(max: Double, fullFrame: Double, min: Double, sdrWhite: Double) {
        self.max = max
        self.fullFrame = fullFrame
        self.min = min
        self.sdrWhite = sdrWhite
    }

    /// Valeurs par défaut, celles du cœur (`kn_hdr_luminance_default` : 1 000,
    /// 1 000, 0, et 203 nits pour le blanc, BT.2408).
    public static var `default`: HDRLuminance { HDRLuminance(kn_hdr_luminance_default()) }

    /// Bornes du cœur (`kn_hdr_luminance_clamp` : max 100 à 10 000, plein
    /// écran ≤ max, noir 0 à 1, blanc 80 à 480).
    public var clamped: HDRLuminance { HDRLuminance(kn_hdr_luminance_clamp(kn)) }

    init(_ l: KnHdrLuminance) {
        self.init(max: l.max, fullFrame: l.full_frame, min: l.min, sdrWhite: l.sdr_white)
    }

    var kn: KnHdrLuminance { KnHdrLuminance(max: max, full_frame: fullFrame, min: min, sdr_white: sdrWhite) }

    /// L'objet JSON du cœur et d'Admin.
    public var json: [String: Any] {
        ["max": max, "full_frame": fullFrame, "min": min, "sdr_white": sdrWhite]
    }

    private enum CodingKeys: String, CodingKey {
        case max, fullFrame = "full_frame", min, sdrWhite = "sdr_white"
    }
}

/// Les valeurs de chaque écran de l'appareil : automatiques, ou réglées à l'œil.
public enum HDRCalibrationStore {
    private static let key = "hdrCalibration"
    /// Préférences où sont gardées les valeurs (la démonstration a les siennes).
    nonisolated(unsafe) public static var defaults: UserDefaults = .standard

    /// Valeurs de cet écran, et si elles ont été réglées à l'œil.
    public static func luminance(for screen: ScreenRef) -> (value: HDRLuminance, calibrated: Bool) {
        if let saved = saved[screen.id] { return (saved.clamped, true) }
        return (automatic(screen), false)
    }

    /// Les valeurs à donner au cœur pour cet écran, s'il est HDR (sinon nil).
    public static func session(_ screen: ScreenRef?, hdr: Bool) -> HDRLuminance? {
        guard hdr, let screen else { return nil }
        return luminance(for: screen).value
    }

    /// Garde les valeurs réglées à l'œil pour cet écran.
    public static func save(_ value: HDRLuminance, for screen: ScreenRef) {
        var all = saved
        all[screen.id] = value.clamped
        write(all)
        KyberLog.app.notice("réglage du HDR gardé pour l'écran \(screen.id, privacy: .public) : \(Int(value.max)) / \(Int(value.fullFrame)) / \(value.min) / \(Int(value.sdrWhite)) nits")
    }

    /// « Revenir au réglage automatique ».
    public static func reset(_ screen: ScreenRef) {
        var all = saved
        all.removeValue(forKey: screen.id)
        write(all)
    }

    private static var saved: [String: HDRLuminance] {
        guard let data = defaults.data(forKey: key) else { return [:] }
        return (try? JSONDecoder().decode([String: HDRLuminance].self, from: data)) ?? [:]
    }

    private static func write(_ all: [String: HDRLuminance]) {
        defaults.set(try? JSONEncoder().encode(all), forKey: key)
    }

    /// Un écran de l'appareil : son identifiant stable, sa marge EDR possible et ce qu'on sait de son modèle.
    public struct ScreenRef: Equatable, Sendable {
        public let id: String
        public let headroom: Double
        /// Écran Apple à rétroéclairage mini-LED ou OLED XDR (1 600 nits en HDR).
        public let xdr: Bool

        public init(id: String, headroom: Double, xdr: Bool) {
            self.id = id
            self.headroom = headroom
            self.xdr = xdr
        }

        #if os(macOS)
        /// L'écran d'un Mac : vendeur, modèle et numéro de série (stables d'un démarrage à l'autre).
        public init(_ screen: NSScreen) {
            let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID ?? 0
            id = "\(CGDisplayVendorNumber(number))-\(CGDisplayModelNumber(number))-\(CGDisplaySerialNumber(number))"
            headroom = HDRDisplay.headroom(screen)
            // Écran intégré d'un MacBook Pro à écran XDR, ou Pro Display XDR.
            let name = screen.localizedName.lowercased()
            xdr = name.contains("xdr") || (CGDisplayIsBuiltin(number) != 0 && headroom >= 8)
        }

        /// L'écran de la fenêtre (sinon l'écran principal).
        public static func current(_ screen: NSScreen?) -> ScreenRef? {
            (screen ?? NSScreen.main).map(ScreenRef.init)
        }
        #elseif os(iOS)
        /// L'écran intégré de l'iPhone ou de l'iPad, ou un écran branché.
        public init(_ screen: UIScreen?) {
            let s = screen ?? UIScreen.main
            let builtin = s == UIScreen.main
            id = builtin ? "builtin" : "external-\(Int(s.nativeBounds.width))x\(Int(s.nativeBounds.height))"
            headroom = HDRDisplay.headroom(s)
            // iPhone et iPad Pro XDR : marge possible de 8 et plus.
            xdr = builtin && headroom >= 8
        }

        public static func current(_ screen: UIScreen?) -> ScreenRef? { ScreenRef(screen) }
        #else
        /// Le téléviseur : un seul écran, sans valeurs données par tvOS.
        public init(_ screen: UIScreen? = nil) {
            id = "tv"
            headroom = HDRDisplay.headroom()
            xdr = false
        }

        public static func current(_ screen: UIScreen? = nil) -> ScreenRef? { ScreenRef() }
        #endif
    }

    /// Valeurs automatiques : un écran XDR d'Apple en a de connues ; un autre
    /// écran à marge EDR, sa marge au-dessus d'un blanc de 300 nits (Studio
    /// Display : marge 2, 600 nits), bornée de 400 à 1 000 ; sinon, par défaut.
    public static func automatic(_ screen: ScreenRef) -> HDRLuminance {
        if screen.xdr { return HDRLuminance(max: 1600, fullFrame: 1000, min: 0, sdrWhite: 203) }
        #if os(tvOS)
        return .default
        #else
        guard screen.headroom > 1 else { return .default }
        let peak = Swift.min(Swift.max(300 * screen.headroom, 400), 1000)
        return HDRLuminance(max: peak, fullFrame: peak, min: 0, sdrWhite: 203)
        #endif
    }
}

/// L'écran « Réglage du HDR » : quatre étapes, la valeur de chacune.
/// L'app dessine le reste (titres, boutons) avec ses composants ; la mire est
/// `HDRPatternView`.
@MainActor
public final class HDRCalibrationModel: ObservableObject {
    public enum Step: Int, CaseIterable, Sendable {
        /// Luminosité maximale : une petite zone claire, un symbole au maximum de l'écran.
        case peak
        /// Luminosité sur tout l'écran : la même mire en plein écran.
        case fullFrame
        /// Noir : un symbole à peine plus clair que le noir.
        case black
        /// Blanc du bureau : une fenêtre et du texte.
        case desktopWhite
    }

    @Published public private(set) var step: Step = .peak
    @Published public private(set) var values: HDRLuminance
    public let screen: HDRCalibrationStore.ScreenRef
    /// Les valeurs étaient réglées à l'œil à l'ouverture.
    public let wasCalibrated: Bool

    public init(screen: HDRCalibrationStore.ScreenRef) {
        self.screen = screen
        let current = HDRCalibrationStore.luminance(for: screen)
        values = current.value
        wasCalibrated = current.calibrated
    }

    /// La valeur de l'étape en cours, en nits.
    public var value: Double {
        switch step {
        case .peak: return values.max
        case .fullFrame: return values.fullFrame
        case .black: return values.min
        case .desktopWhite: return values.sdrWhite
        }
    }

    public var isFirst: Bool { step == .peak }
    public var isLast: Bool { step == .desktopWhite }

    /// Un cran vers le haut (+1) ou le bas (−1) : flèches, molette, télécommande, manette.
    public func adjust(_ direction: Int, fine: Bool = false) {
        guard direction != 0 else { return }
        let up = direction > 0
        var v = values
        switch step {
        case .peak: v.max = Self.ladder(v.max, up: up, fine: fine)
        case .fullFrame: v.fullFrame = Swift.min(Self.ladder(v.fullFrame, up: up, fine: fine), v.max)
        case .black: v.min = (v.min + (up ? 1 : -1) * (fine ? 0.001 : 0.005) * 1000).rounded() / 1000
        case .desktopWhite: v.sdrWhite += (up ? 1 : -1) * (fine ? 2 : 10)
        }
        values = v.clamped
    }

    /// Valeur choisie directement (glisser au toucher), bornée.
    public func set(_ value: Double) {
        var v = values
        switch step {
        case .peak: v.max = value.rounded()
        case .fullFrame: v.fullFrame = Swift.min(value.rounded(), v.max)
        case .black: v.min = (value * 1000).rounded() / 1000
        case .desktopWhite: v.sdrWhite = value.rounded()
        }
        values = v.clamped
    }

    /// Bornes de l'étape, pour un curseur (logarithmique pour les maxima).
    public var range: ClosedRange<Double> {
        switch step {
        case .peak: return 100...10000
        case .fullFrame: return 100...Swift.max(values.max, 100)
        case .black: return 0...1
        case .desktopWhite: return 80...480
        }
    }

    public func next() {
        if let s = Step(rawValue: step.rawValue + 1) { step = s }
    }

    public func back() {
        if let s = Step(rawValue: step.rawValue - 1) { step = s }
    }

    /// « Terminer » : les valeurs sont gardées pour cet écran.
    public func done() {
        HDRCalibrationStore.save(values, for: screen)
    }

    /// « Revenir au réglage automatique » : les valeurs réglées sont oubliées.
    public func resetToAutomatic() {
        HDRCalibrationStore.reset(screen)
        values = HDRCalibrationStore.automatic(screen)
    }

    /// Crans de 4 % (1 % en fin), arrondis à 10 nits (à 1 en fin).
    private static func ladder(_ v: Double, up: Bool, fine: Bool) -> Double {
        let f = fine ? 1.01 : 1.04
        let next = up ? v * f : v / f
        let step = fine ? 1.0 : 10.0
        let rounded = (next / step).rounded() * step
        return rounded == v ? v + (up ? step : -step) : rounded
    }
}
