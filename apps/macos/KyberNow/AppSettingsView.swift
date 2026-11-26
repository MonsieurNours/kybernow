// Réglages de l'app (KyberNow Link › Réglages…, ⌘, ; « Réglages de l'app » en
// haut de l'accueil) : ce qui vaut pour tous les ordinateurs, et ce que ce Mac
// sait faire. Les réglages de chaque ordinateur, eux, vont par ordinateur.

import KyberKit
import ServiceManagement
import SwiftUI

struct AppSettingsView: View {
    @ObservedObject var cloud: CloudPublisher
    /// Comptes KyberNow (Accounts.swift).
    @ObservedObject var accounts: AccountStore
    /// Bêtas proposées par Sparkle (voir `UpdateChannels`).
    @AppStorage(UpdateChannels.betaKey) private var betaUpdates = false
    /// « Ouvrir à l'ouverture de session » (AppLife.swift).
    @ObservedObject private var login = LoginItem.shared
    /// Langue de l'app (`AppleLanguages` de KyberNow), lue au lancement.
    @State private var language: LanguageChoice? = LanguageChoice.current
    /// Relu quand un réglage du HDR est gardé ou remis en automatique.
    @State private var calibrationTick = 0
    /// Langue en vigueur depuis le lancement (le choix ne s'applique qu'à la relance).
    private static let launchLanguage = LanguageChoice.current

    var body: some View {
        VStack(spacing: 16) {
            AccountsCard(accounts: accounts)
            iCloudCard
            startupCard
            updatesCard
            languageCard
            macCard
        }
        .padding(20)
        .frame(width: 500)
        .background(Theme.background)
        .preferredColorScheme(.dark)
        // Réglages Système a pu le changer : relu à chaque ouverture et retour dans l'app.
        .onAppear { login.refresh() }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in login.refresh() }
    }

    /// L'app démarre avec le Mac, sans fenêtre, dans la barre des menus.
    private var startupCard: some View {
        SettingsCard(icon: "power", title: String(localized: "Démarrage")) {
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Ouvrir à l'ouverture de session").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                    Text("KyberNow Link démarre avec votre Mac, sans fenêtre : vos ordinateurs vous attendent dans la barre des menus.")
                        .font(Theme.sans(12)).foregroundStyle(Theme.muted).lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                Switch(isOn: Binding(get: { login.enabled }, set: { login.set($0) }), label: String(localized: "Ouvrir à l'ouverture de session"))
            }
            if login.needsApproval {
                Button { SMAppService.openSystemSettingsLoginItems() } label: {
                    Text("À autoriser dans Réglages Système › Général.")
                        .font(Theme.sans(12)).foregroundStyle(Theme.warnText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .buttonStyle(.plain)
            }
        }
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
            hdrCalibrationRow
        }
        .onReceive(NotificationCenter.default.publisher(for: HDRCalibrationWindow.changed)) { _ in calibrationTick += 1 }
    }

    /// « Réglage du HDR », seulement sur un écran HDR : l'état de cet écran
    /// (« Automatique » ou « Réglé à l'œil »), et l'écran du réglage.
    @ViewBuilder private var hdrCalibrationRow: some View {
        let screen = NSApp.keyWindow?.screen ?? NSScreen.main
        if HDRDisplay.isHDR(screen), let ref = HDRCalibrationStore.ScreenRef.current(screen) {
            let calibrated = HDRCalibrationStore.luminance(for: ref).calibrated
            Button { HDRCalibrationWindow.open(on: screen) } label: {
                HStack(spacing: 12) {
                    Icon("sparkle", size: 17).foregroundStyle(Theme.accent)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(HDRText.calibration).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                        Text(HDRText.calibrationState(calibrated)).font(Theme.sans(12)).foregroundStyle(Theme.muted)
                    }
                    Spacer(minLength: 12)
                    Icon("arrow-right", size: 14, bold: true).foregroundStyle(Theme.faint)
                }
                .padding(.horizontal, 12)
                .frame(height: 50)
                .background(Color.white.opacity(0.04), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Theme.line2))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .id(calibrationTick)
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
                    Text("Ordinateurs et consoles").font(Theme.sans(12, .semibold)).foregroundStyle(Theme.text2)
                    Text("Vos ordinateurs et leurs accès arrivent sur l'Apple TV, l'iPhone et l'iPad connectés au même compte iCloud ; vos comptes PlayStation et leurs consoles, sur tous vos appareils Apple. Sans rien taper. Mots de passe et associations dans le Trousseau iCloud, chiffré de bout en bout.")
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

// MARK: - Comptes KyberNow

/// « Comptes » (docs/link-ui.md, « Comptes KyberNow ») : chaque compte (nom,
/// e-mail, relais), « Gérer le compte » (la page du compte, dans le
/// navigateur), « Se déconnecter » (confirmé) ; « Ajouter un compte » : le
/// relais d'abord, puis la page de connexion du relais dans la fenêtre du
/// système. Créer, changer le mot de passe, supprimer : sur la page du compte.
private struct AccountsCard: View {
    @ObservedObject var accounts: AccountStore
    @StateObject private var relay = AccountRelayModel()
    /// « Ajouter un compte » ouvert : le choix du relais, puis le champ d'« Un autre relais… ».
    @State private var adding = false
    @State private var other = false
    /// « Se déconnecter », à confirmer.
    @State private var signingOut: KyberAccount?
    @Environment(\.openURL) private var openURL

    var body: some View {
        SettingsCard(icon: "user", title: AccountText.section) {
            if accounts.accounts.isEmpty {
                Text(AccountText.none).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
            }
            ForEach(accounts.accounts) { a in
                HStack(alignment: .top, spacing: 12) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(verbatim: a.displayName).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                        if !a.name.isEmpty, !a.email.isEmpty {
                            Text(verbatim: a.email).font(Theme.sans(12)).foregroundStyle(Theme.text2)
                        }
                        Text(verbatim: a.relayLabel).font(Theme.mono(11)).foregroundStyle(Theme.muted)
                    }
                    Spacer(minLength: 8)
                    Button(AccountText.manage) {
                        if let url = URL(string: a.console) { openURL(url) }
                    }
                    .buttonStyle(GhostButton(height: 30))
                    Button(AccountText.signOut) { signingOut = a }
                        .buttonStyle(GhostButton(height: 30))
                }
                .padding(12)
                .background(Color.white.opacity(0.04), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Theme.line2))
            }
            if accounts.signingIn {
                HStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text(AccountText.signingIn).font(Theme.sans(12)).foregroundStyle(Theme.muted)
                }
            } else if !adding {
                Button(accounts.accounts.isEmpty ? AccountText.signIn : AccountText.add) {
                    adding = true
                    other = false
                }
                .buttonStyle(GhostButton(height: 34))
            }
            if adding, !accounts.signingIn { relayChoice }
            // Réglages synchronisés : seulement avec un compte connecté.
            if !accounts.accounts.isEmpty {
                HStack(alignment: .top, spacing: 16) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(verbatim: Accounts.syncSettingsTitle).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                        Text(verbatim: Accounts.syncSettingsHelp)
                            .font(Theme.sans(12)).foregroundStyle(Theme.muted).lineSpacing(2)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    Spacer(minLength: 0)
                    Switch(isOn: $accounts.syncSettings, label: Accounts.syncSettingsTitle)
                }
            }
            Text(AccountText.sectionHelp)
                .font(Theme.sans(12)).foregroundStyle(Theme.faint)
                .fixedSize(horizontal: false, vertical: true)
            // L'accueil du premier lancement, rejoué sur la fenêtre de l'accueil (Welcome.swift).
            Button(WelcomeText.replay) {
                let app = AppModel.shared
                NSApp.keyWindow?.close()
                app.route = .home
                app.showWindow()
                app.welcome.replay()
            }
            .buttonStyle(GhostButton(height: 30))
            .fixedSize()
        }
        .confirmationDialog(AccountText.signOutQuestion(signingOut?.displayName ?? ""),
                            isPresented: Binding(get: { signingOut != nil }, set: { if !$0 { signingOut = nil } }), titleVisibility: .visible) {
            Button(AccountText.signOut, role: .destructive) {
                if let a = signingOut { accounts.signOut(a) }
                signingOut = nil
            }
            Button(StationText.cancel, role: .cancel) { signingOut = nil }
        }
        .onChange(of: relay.text) { _, _ in relay.changed() }
    }

    /// Le relais d'abord : share.kybernow.app proposé, ou « Un autre relais… ».
    @ViewBuilder private var relayChoice: some View {
        if other {
            VStack(alignment: .leading, spacing: 8) {
                Text(AccountText.otherRelayTitle).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                Text(AccountText.relayField).font(Theme.sans(12, .semibold)).foregroundStyle(Theme.text2)
                TextField(text: $relay.text, prompt: Text(verbatim: AccountText.relayPlaceholder).foregroundColor(Theme.ghost)) { EmptyView() }
                    .textFieldStyle(.plain)
                    .font(Theme.mono(13))
                    .foregroundStyle(Theme.text)
                    .autocorrectionDisabled()
                    .padding(.horizontal, 12)
                    .frame(height: 36)
                    .background(Theme.bar, in: RoundedRectangle(cornerRadius: 9))
                    .overlay(RoundedRectangle(cornerRadius: 9).stroke(Theme.line2))
                    .onSubmit { continueWithOther() }
                Text(AccountText.relayHelp).font(Theme.sans(12)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
                if let failure = relay.failure {
                    Text(failure).font(Theme.sans(12)).foregroundStyle(Theme.warnText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                HStack(spacing: 10) {
                    Button(AccountText.continueLabel) { continueWithOther() }
                        .buttonStyle(PrimaryButton(height: 34))
                        .disabled(!relay.canContinue)
                    Button(StationText.cancel) {
                        other = false
                        relay.changed()
                    }
                    .buttonStyle(GhostButton(height: 34))
                }
            }
        } else {
            HStack(spacing: 10) {
                Button { start("") } label: { Text(verbatim: Accounts.defaultRelay) }
                    .buttonStyle(PrimaryButton(height: 34))
                Button(AccountText.otherRelay) { other = true }
                    .buttonStyle(GhostButton(height: 34))
                Button(StationText.cancel) { adding = false }
                    .buttonStyle(GhostButton(height: 34))
            }
            Text(AccountText.signInHelpWindow).font(Theme.sans(12)).foregroundStyle(Theme.muted)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func continueWithOther() {
        guard relay.canContinue, let address = relay.validate() else { return }
        start(address)
    }

    private func start(_ address: String) {
        adding = false
        other = false
        relay.text = ""
        accounts.signIn(relay: address)
    }
}
