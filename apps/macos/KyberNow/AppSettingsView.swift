// Réglages de l'app (KyberNow > Réglages…, ⌘,) : ce qui vaut pour tous les
// ordinateurs. Les réglages de connexion, eux, vont par ordinateur.

import KyberKit
import SwiftUI

struct AppSettingsView: View {
    @ObservedObject var cloud: CloudPublisher
    /// Bêtas proposées par Sparkle (voir `UpdateChannels`).
    @AppStorage(UpdateChannels.betaKey) private var betaUpdates = false
    /// Langue de l'app (`AppleLanguages` de KyberNow), lue au lancement.
    @State private var language: LanguageChoice? = LanguageChoice.current
    /// Langue en vigueur depuis le lancement (le choix ne s'applique qu'à la relance).
    private static let launchLanguage = LanguageChoice.current

    var body: some View {
        VStack(spacing: 16) {
            iCloudCard
            updatesCard
            languageCard
        }
        .padding(20)
        .frame(width: 480)
        .background(Theme.background)
        .preferredColorScheme(.dark)
    }

    private var updatesCard: some View {
        SettingsCard(icon: "arrows-clockwise", title: String(localized: "Mises à jour")) {
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Recevoir les versions bêta").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                    Text("Les nouveautés avant tout le monde, parfois moins stables. Une version stable plus récente vous est toujours proposée ; décocher ne fait pas revenir en arrière, KyberNow Link attend la stable suivante.")
                        .font(Theme.sans(12)).foregroundStyle(Theme.muted).lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                Switch(isOn: $betaUpdates, label: String(localized: "Recevoir les versions bêta"))
            }
        }
    }

    private var languageCard: some View {
        SettingsCard(icon: "globe-hemisphere-west", title: String(localized: "Langue")) {
            Segmented(options: LanguageChoice.allCases.map { ($0, $0.label) }, selection: $language, fill: true)
                .onChange(of: language) { _, choice in
                    if let choice { LanguageChoice.current = choice }
                }
            Text(language != Self.launchLanguage
                 ? "Relancez KyberNow Link pour changer de langue."
                 : "Auto : la langue de macOS. Ce choix ne vaut que pour KyberNow Link.")
                .font(Theme.sans(12))
                .foregroundStyle(language != Self.launchLanguage ? Theme.warnText : Theme.faint)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var iCloudCard: some View {
        SettingsCard(icon: "cloud", title: "iCloud") {
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Synchroniser avec iCloud").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                    Text("Vos ordinateurs et leurs accès arrivent sur l'Apple TV connectée au même compte iCloud, sans rien taper. Mots de passe dans le Trousseau iCloud, chiffré de bout en bout.")
                        .font(Theme.sans(12)).foregroundStyle(Theme.muted).lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                Switch(isOn: $cloud.enabled, label: String(localized: "Synchroniser avec iCloud"))
            }
            if let status {
                Text(status.text)
                    .font(Theme.sans(12))
                    .foregroundStyle(status.warning ? Theme.warnText : Theme.faint)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var status: (text: String, warning: Bool)? {
        switch cloud.state {
        case .off:
            return nil
        case .unavailable:
            return (String(localized: "Cette version de KyberNow Link n'est pas signée pour iCloud : rien n'est publié."), true)
        case .failed(let message):
            return (message, true)
        case .published(let n):
            if cloud.signedOut { return (String(localized: "Aucun compte iCloud sur ce Mac : la liste partira à la connexion."), true) }
            let text = n == 0 ? String(localized: "Aucun ordinateur à publier pour l'instant.")
                : n == 1 ? String(localized: "1 ordinateur publié.")
                : String(localized: "\(n) ordinateurs publiés.")
            return (text, false)
        }
    }
}
