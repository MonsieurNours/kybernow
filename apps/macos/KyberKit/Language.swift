// Langue de l'app (docs/i18n.md) : français si la langue retenue par le
// système pour l'app est le français, anglais sinon. Les textes sont dans
// les catalogues Localizable.xcstrings (français source, traduction anglaise) ;
// CFBundleDevelopmentRegion vaut `en`, donc toute autre langue retombe sur
// l'anglais.

import Foundation

private final class KyberKitBundleToken {}

extension Bundle {
    /// Ressources de KyberKit (son catalogue de chaînes), pour
    /// `String(localized: "…", bundle: .kyberKit)`.
    public static let kyberKit = Bundle(for: KyberKitBundleToken.self)
}

/// Choix de la langue dans les réglages de l'app : Auto (langues du
/// système), Français ou English. Il s'applique à cette app seulement, par
/// sa préférence `AppleLanguages`, lue au lancement : il faut relancer.
public enum LanguageChoice: String, CaseIterable, Identifiable, Sendable {
    case auto, fr, en

    public var id: String { rawValue }

    /// Nom affiché dans le sélecteur : chaque langue dans sa propre langue.
    public var label: String {
        switch self {
        case .auto: return String(localized: "Auto", bundle: .kyberKit)
        case .fr: return "Français"
        case .en: return "English"
        }
    }

    private static let key = "AppleLanguages"

    /// Choix enregistré : seulement la préférence propre à l'app, pas celle
    /// du système (que `UserDefaults.standard` renverrait aussi).
    public static var current: LanguageChoice {
        get {
            guard let id = Bundle.main.bundleIdentifier,
                  let langs = UserDefaults.standard.persistentDomain(forName: id)?[key] as? [String],
                  let first = langs.first
            else { return .auto }
            return first.hasPrefix("fr") ? .fr : .en
        }
        set {
            switch newValue {
            case .auto: UserDefaults.standard.removeObject(forKey: key)
            case .fr, .en: UserDefaults.standard.set([newValue.rawValue], forKey: key)
            }
        }
    }

    /// Langue de l'interface en ce moment (`fr` ou `en`) : celle que le
    /// système a retenue pour l'app au lancement. Envoyée à la console
    /// (`Accept-Language`) pour qu'elle réponde dans la même langue.
    public static var effective: String {
        (Bundle.main.preferredLocalizations.first ?? "en").hasPrefix("fr") ? "fr" : "en"
    }
}
