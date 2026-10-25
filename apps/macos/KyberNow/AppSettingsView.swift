// Réglages de l'app (KyberNow Link › Réglages…, ⌘, ; « Réglages de l'app » en
// haut de l'accueil) : ce qui vaut pour tous les ordinateurs, et ce que ce Mac
// sait faire. Les réglages de chaque ordinateur, eux, vont par ordinateur.

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
            macCard
        }
        .padding(20)
        .frame(width: 500)
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

    /// Ce que KyberNow Link a mesuré de ce Mac : les profils s'y adaptent.
    private var macCard: some View {
        let screen = NSScreen.main ?? NSScreen.screens[0]
        let native = DisplayInfo.nativePixels(screen)
        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "—"
        return SettingsCard(icon: "cpu", title: String(localized: "Ce Mac")) {
            VStack(alignment: .leading, spacing: 8) {
                fact(String(localized: "Écran"), "\(screen.localizedName) · \(LinkFormat.dims(native.width, native.height)) · \(DisplayInfo.refreshLabel(screen))", mono: false)
                if let looks = DisplayInfo.scaledLooksLike(screen) {
                    fact(String(localized: "Affichage de macOS"), String(localized: "agrandi, comme \(looks)"), mono: false)
                }
                fact(String(localized: "Décodage matériel"), DecoderCapabilities.current.summary, mono: false)
                fact(String(localized: "Version"), "KyberNow Link \(version)", mono: false)
            }
            Text(DisplayInfo.variableRefresh(screen)
                 ? "L'écran se cale sur chaque image : latence minimale sans déchirure."
                 : "Les profils demandent l'image à la fréquence de cet écran.")
                .font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            GamepadStatus()
        }
    }

    private func fact(_ label: String, _ value: String, mono: Bool = true) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).font(Theme.sans(13)).foregroundStyle(Theme.muted).fixedSize()
            Spacer(minLength: 12)
            Text(verbatim: value).font(mono ? Theme.mono(12) : Theme.sans(13)).foregroundStyle(Theme.text)
                .multilineTextAlignment(.trailing).fixedSize(horizontal: false, vertical: true)
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
                    // Ordinateurs de démonstration : rien ne part vers iCloud.
                    .disabled(AppModel.demo)
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
