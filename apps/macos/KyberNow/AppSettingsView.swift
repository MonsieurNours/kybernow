// Réglages de l'app (KyberNow > Réglages…, ⌘,) : ce qui vaut pour tous les
// ordinateurs. Les réglages de connexion, eux, vont par ordinateur.

import SwiftUI

struct AppSettingsView: View {
    @ObservedObject var cloud: CloudPublisher
    /// Bêtas proposées par Sparkle (voir `UpdateChannels`).
    @AppStorage(UpdateChannels.betaKey) private var betaUpdates = false

    var body: some View {
        VStack(spacing: 16) {
            iCloudCard
            updatesCard
        }
        .padding(20)
        .frame(width: 480)
        .background(Theme.background)
        .preferredColorScheme(.dark)
    }

    private var updatesCard: some View {
        SettingsCard(icon: "arrows-clockwise", title: "Mises à jour") {
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Recevoir les versions bêta").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                    Text("Les nouveautés avant tout le monde, parfois moins stables. Une version stable plus récente vous est toujours proposée ; décocher ne fait pas revenir en arrière, KyberNow attend la stable suivante.")
                        .font(Theme.sans(12)).foregroundStyle(Theme.muted).lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                Switch(isOn: $betaUpdates, label: "Recevoir les versions bêta")
            }
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
                Switch(isOn: $cloud.enabled, label: "Synchroniser avec iCloud")
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
            return ("Cette version de KyberNow n'est pas signée pour iCloud : rien n'est publié.", true)
        case .failed(let message):
            return (message, true)
        case .published(let n):
            if cloud.signedOut { return ("Aucun compte iCloud sur ce Mac : la liste partira à la connexion.", true) }
            return (n == 0 ? "Aucun ordinateur à publier pour l'instant." : n == 1 ? "1 ordinateur publié." : "\(n) ordinateurs publiés.", false)
        }
    }
}
