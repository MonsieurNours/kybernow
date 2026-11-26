// Réglages de l'app (fiche) : ce qui vaut sur cet appareil pour tous les
// ordinateurs : iCloud, langue, données mobiles, ce que l'appareil sait faire
// (écran, décodage, sortie du son), la version. Les réglages d'un ordinateur
// (image, son, commandes, réseau, statistiques, Station), eux, vont par
// ordinateur (SettingsView) : chaque réglage à un seul endroit. Même langage
// que les réglages par ordinateur (maquette « Réglages », n7a) : des lignes de
// verre, des choix en pastilles, des interrupteurs Spectre.
//
// On y arrive par le bouton « Réglages de l'app » de l'accueil.

import AVFoundation
import KyberKit
import SwiftUI

struct AppSettingsView: View {
    @EnvironmentObject private var app: AppModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        AppSettingsContent(app: app, cloud: app.cloud, accounts: app.accounts, close: { dismiss() })
            .presentationDragIndicator(.visible)
            .presentationBackground(Theme.background)
    }
}

private struct AppSettingsContent: View {
    let app: AppModel
    @ObservedObject var cloud: CloudSync
    /// Comptes KyberNow (Accounts.swift).
    @ObservedObject var accounts: AccountStore
    let close: () -> Void
    @ObservedObject private var prefs = Preferences.shared
    /// Langue choisie ici ; celle du lancement reste affichée jusqu'à la relance.
    @State private var language = LanguageChoice.current
    private static let launchLanguage = LanguageChoice.current
    @State private var route = AudioRoute.current
    @StateObject private var link = SettingsLink()
    /// L'écran « Réglage du HDR » ouvert.
    @State private var calibrating = false
    /// « Ajouter un compte » : le relais d'abord.
    @State private var addingAccount = false
    /// « Se déconnecter », à confirmer.
    @State private var signingOut: KyberAccount?
    @Environment(\.openURL) private var openURL

    private var isPad: Bool { DeviceDisplay.isPad }

    var body: some View {
        ZStack(alignment: .top) {
            backdrop
            ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 8) {
                    header
                    section(AccountText.section) { accountRows }
                    section(String(localized: "iCloud")) { icloudRows }
                    section(String(localized: "Langue")) { languageRow }
                    section(String(localized: "Réseau")) { networkRows }
                    section(isPad ? String(localized: "Cet iPad") : String(localized: "Cet iPhone")) { deviceRows }
                    // Réglage du HDR : seulement sur un écran HDR ; les valeurs sont celles de l'appareil.
                    if HDRDisplay.isHDR(nil) {
                        section(HDRText.label) { calibrationRow }
                            .id("hdrCalibration")
                    }
                    section(String(localized: "À propos")) { aboutRows }
                }
                .padding(.horizontal, 24)
                .padding(.top, 8)
                .padding(.bottom, 40)
                .frame(maxWidth: 640)
                .frame(maxWidth: .infinity)
            }
            .scrollIndicators(.hidden)
            .safeAreaInset(edge: .top, spacing: 0) { navigation }
            .onAppear {
                #if DEBUG
                // Captures (`-scrollTo hdrCalibration`) : la fiche défile jusqu'à la ligne voulue.
                if let target = UserDefaults.standard.string(forKey: "scrollTo") {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { proxy.scrollTo(target, anchor: .center) }
                }
                #endif
            }
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: AVAudioSession.routeChangeNotification).receive(on: DispatchQueue.main)) { _ in
            route = AudioRoute.current
        }
        .hdrCalibrationCover(isPresented: $calibrating)
        .sheet(isPresented: $addingAccount) {
            AddAccountSheet(accounts: accounts)
                .presentationDragIndicator(.visible)
                .presentationBackground(Theme.background)
        }
        .confirmationDialog(AccountText.signOutQuestion(signingOut?.displayName ?? ""),
                            isPresented: Binding(get: { signingOut != nil }, set: { if !$0 { signingOut = nil } }), titleVisibility: .visible) {
            Button(AccountText.signOut, role: .destructive) {
                if let a = signingOut { accounts.signOut(a) }
                signingOut = nil
            }
            Button(StationText.cancel, role: .cancel) { signingOut = nil }
        }
    }

    // MARK: Comptes KyberNow

    /// Chaque compte (nom, e-mail, relais), « Gérer le compte » (la page du
    /// compte) et « Se déconnecter » ; « Ajouter un compte ». Créer, changer
    /// le mot de passe, supprimer : sur la page du compte, pas dans l'app.
    @ViewBuilder private var accountRows: some View {
        if accounts.accounts.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                Text(AccountText.none).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
                Button { accounts.signIn() } label: {
                    HStack(spacing: 10) {
                        if accounts.signingIn { HomeSpinner(size: 14).frame(width: 16, height: 16) }
                        Text(accounts.signingIn ? AccountText.signingIn : AccountText.signIn)
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(PrimaryButton(height: 48, fontSize: 15))
                .disabled(accounts.signingIn)
                Text(AccountText.signInHelpWindow).font(Theme.sans(12)).foregroundStyle(Theme.muted)
                    .lineSpacing(2).fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .modifier(SettingsRowBackground())
        } else {
            ForEach(accounts.accounts) { a in
                VStack(alignment: .leading, spacing: 10) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(verbatim: a.displayName).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
                        if !a.name.isEmpty, !a.email.isEmpty {
                            Text(verbatim: a.email).font(Theme.sans(12.5)).foregroundStyle(Theme.text2)
                        }
                        Text(verbatim: a.relayLabel).font(Theme.mono(12)).foregroundStyle(Theme.muted)
                    }
                    .accessibilityElement(children: .combine)
                    HStack(spacing: 8) {
                        Button(AccountText.manage) {
                            if let url = URL(string: a.console) { openURL(url) }
                        }
                        .buttonStyle(GlassButton(height: 40, fontSize: 14))
                        Button(AccountText.signOut) { signingOut = a }
                            .buttonStyle(GlassButton(height: 40, fontSize: 14))
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .modifier(SettingsRowBackground())
            }
            if accounts.signingIn {
                HStack(spacing: 10) {
                    HomeSpinner(size: 13).foregroundStyle(Theme.accentLight).frame(width: 16, height: 16)
                    Text(AccountText.signingIn).font(Theme.sans(13)).foregroundStyle(Theme.muted)
                }
                .padding(.horizontal, 14)
            }
        }
        // Réglages synchronisés : seulement avec un compte connecté.
        if !accounts.accounts.isEmpty {
            SettingsToggleRow(title: Accounts.syncSettingsTitle, note: Accounts.syncSettingsHelp, on: accounts.syncSettings) {
                accounts.syncSettings.toggle()
            }
        }
        Button { addingAccount = true } label: {
            HStack(spacing: 12) {
                SettingsLabel(title: AccountText.add, note: AccountText.sectionHelp)
                Icon("plus", size: 18).foregroundStyle(Theme.text2)
            }
            .modifier(SettingsRowBackground())
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(accounts.signingIn)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        // L'accueil du premier lancement, rejoué (Welcome.swift).
        Button {
            app.sheet = nil
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [app] in app.welcome.replay() }
        } label: {
            HStack(spacing: 12) {
                Text(WelcomeText.replay).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Icon("arrow-right", size: 18).foregroundStyle(Theme.text2)
            }
            .modifier(SettingsRowBackground())
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    /// « Réglage du HDR », avec l'état en dessous : « Automatique » ou « Réglé à l'œil ».
    private var calibrationRow: some View {
        let calibrated = HDRCalibrationStore.luminance(for: HDRCalibrationStore.ScreenRef(UIScreen.main)).calibrated
        return Button { calibrating = true } label: {
            HStack(spacing: 12) {
                SettingsLabel(title: HDRText.calibration, note: HDRText.calibrationState(calibrated))
                Icon("arrow-right", size: 18).foregroundStyle(Theme.text2)
            }
            .modifier(SettingsRowBackground())
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .id(calibrating)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
    }

    // MARK: Cadre

    private var backdrop: some View {
        ZStack {
            Theme.background
            RadialGradient(colors: [Theme.rgb(0x221B55), Theme.rgb(0x221B55, 0)],
                           center: UnitPoint(x: 0.9, y: 0), startRadius: 0, endRadius: 420)
                .blur(radius: 30)
        }
        .ignoresSafeArea()
        .accessibilityHidden(true)
    }

    private var navigation: some View {
        HStack {
            Text((isPad ? String(localized: "Sur cet iPad") : String(localized: "Sur cet iPhone")).uppercased())
                .font(Theme.mono(10.5, .medium)).tracking(0.85).foregroundStyle(Theme.faint)
            Spacer()
            Button(action: close) {
                Icon("x", size: 18, bold: true)
                    .foregroundStyle(Theme.text)
                    .frame(width: 44, height: 44)
                    .glass(22)
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .keyboardShortcut(.cancelAction)
            .accessibilityLabel(Text("Fermer"))
        }
        .padding(.leading, 24).padding(.trailing, 16)
        .padding(.top, 14).padding(.bottom, 6)
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Réglages de l'app")
                .font(Theme.display(34, 660))
                .tracking(-34 * 0.03)
                .foregroundStyle(Theme.text)
                .accessibilityAddTraits(.isHeader)
            Text("Pour tous vos ordinateurs. L'image, le son, les commandes et le profil se règlent par ordinateur.")
                .font(Theme.sans(13.5))
                .foregroundStyle(Theme.muted)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.bottom, 10)
    }

    private func section<Content: View>(_ title: String, @ViewBuilder _ content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title.uppercased())
                .font(Theme.mono(10.5, .medium)).tracking(0.85).foregroundStyle(Theme.muted)
                .padding(.top, 16).padding(.bottom, 2)
                .accessibilityAddTraits(.isHeader)
            content()
        }
    }

    // MARK: Sections

    @ViewBuilder private var icloudRows: some View {
        SettingsToggleRow(title: String(localized: "Synchroniser avec iCloud"),
                          note: String(localized: "Ordinateurs et consoles : vos ordinateurs et leurs accès, vos comptes PlayStation et leurs consoles suivent votre Mac et vos autres appareils, sans rien taper. Mots de passe et associations dans le Trousseau iCloud, chiffré de bout en bout."),
                          on: prefs.icloudSync) {
            prefs.icloudSync.toggle()
            // Activée : ceux déjà publiés arrivent tout de suite, sans attendre un changement d'iCloud.
            if prefs.icloudSync { for c in cloud.hosts { _ = app.addFromCloud(c) } }
        }
        if prefs.icloudSync {
            let n = cloud.hosts.count
            Text(n == 0 ? String(localized: "Aucun ordinateur dans iCloud pour l'instant.")
                 : n == 1 ? String(localized: "1 ordinateur dans iCloud.")
                 : String(localized: "\(n) ordinateurs dans iCloud."))
                .font(Theme.sans(12)).foregroundStyle(Theme.faint)
                .padding(.horizontal, 14)
        }
    }

    private var languageRow: some View {
        // Langue de l'app (docs/i18n.md) : prise en compte à la relance.
        SettingsChoiceRow(title: String(localized: "Langue de KyberNow Link"),
                          note: language == Self.launchLanguage
                              ? (isPad ? String(localized: "Auto : la langue de l'iPad ; pour KyberNow Link seulement.") : String(localized: "Auto : la langue de l'iPhone ; pour KyberNow Link seulement."))
                              : String(localized: "Relancez KyberNow Link pour changer de langue."),
                          warn: language != Self.launchLanguage,
                          options: LanguageChoice.allCases.map { ($0.rawValue, $0.label) },
                          current: language.rawValue) { v in
            language = LanguageChoice(rawValue: v) ?? .auto
            LanguageChoice.current = language
        }
    }

    /// Ce que l'appareil sait faire : les profils s'y adaptent.
    @ViewBuilder private var deviceRows: some View {
        let n = DeviceDisplay.native()
        let caps = DecoderCapabilities.current
        SettingsFactRow(title: String(localized: "Écran"),
                        value: "\(LinkFormat.dims(n.width, n.height)) · \(n.refresh) Hz",
                        note: String(localized: "« Comme cet écran » demande cette définition à l'ordinateur."))
        SettingsFactRow(title: String(localized: "Décodage matériel"),
                        value: caps.summary,
                        note: caps.hevc444 ? String(localized: "Texte net (4:4:4) testé au lancement : oui.") : String(localized: "Texte net (4:4:4) testé au lancement : non."))
        SettingsFactRow(title: String(localized: "Sortie du son"),
                        value: "\(route.name) · \(route.channelsLabel) · \(route.latencyMs) ms",
                        note: String(localized: "Retard mesuré, mis à jour à chaque changement de sortie."),
                        warn: route.isLate)
        SettingsFactRow(title: String(localized: "Réseau"), value: link.label, note: String(localized: "Le lien de cet appareil en ce moment."))
    }

    private var networkRows: some View {
        SettingsToggleRow(title: String(localized: "Sessions en données mobiles"),
                          note: prefs.allowCellular
                              ? String(localized: "Une session peut consommer plusieurs gigaoctets par heure sur votre forfait.")
                              : String(localized: "En données mobiles, KyberNow Link demande avant de se connecter."),
                          warn: prefs.allowCellular,
                          on: prefs.allowCellular) { prefs.allowCellular.toggle() }
    }

    private var aboutRows: some View {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? "—"
        let build = info?["CFBundleVersion"] as? String ?? ""
        return SettingsValueRow(title: String(localized: "Version"),
                                note: String(localized: "KyberNow Link pour l'iPhone et l'iPad"),
                                value: build.isEmpty ? version : "\(version) (\(build))")
    }
}

// MARK: - Lignes

/// Titre et note d'une ligne de réglage.
private struct SettingsLabel: View {
    let title: String
    let note: String
    var warn = false

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
            Text(note)
                .font(Theme.sans(12))
                .foregroundStyle(warn ? Theme.warnText : Theme.muted)
                .lineSpacing(2)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Fond d'une ligne : verre très léger, filet clair.
private struct SettingsRowBackground: ViewModifier {
    func body(content: Content) -> some View {
        content
            .padding(.horizontal, 14).padding(.vertical, 10)
            .frame(minHeight: 60)
            .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Color.white.opacity(0.045)))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Color.white.opacity(0.07)))
    }
}

/// Interrupteur : Spectre allumé, encre éteinte ; toute la ligne se touche.
private struct SettingsToggleRow: View {
    let title: String
    let note: String
    var warn = false
    let on: Bool
    let flip: () -> Void

    var body: some View {
        Button(action: flip) {
            HStack(spacing: 12) {
                SettingsLabel(title: title, note: note, warn: warn)
                ZStack(alignment: on ? .trailing : .leading) {
                    Capsule().fill(on ? Theme.accentFill : Color.white.opacity(0.18))
                    Circle().fill(Color.white).frame(width: 24, height: 24).padding(3)
                }
                .frame(width: 50, height: 30)
                .animation(Theme.easeOut, value: on)
            }
            .modifier(SettingsRowBackground())
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(title))
        .accessibilityHint(Text(note))
        .accessibilityValue(on ? Text("Activé") : Text("Désactivé"))
        .accessibilityAddTraits(.isToggle)
    }
}

/// Choix en pastilles : le retenu en givre plein.
private struct SettingsChoiceRow: View {
    let title: String
    let note: String
    var warn = false
    let options: [(key: String, label: String)]
    let current: String
    let pick: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            SettingsLabel(title: title, note: note, warn: warn)
            HStack(spacing: 6) {
                ForEach(options, id: \.key) { o in
                    let on = o.key == current
                    Button { pick(o.key) } label: {
                        Text(o.label)
                            .font(Theme.sans(13.5, .semibold))
                            .foregroundStyle(on ? Theme.background : Theme.text2)
                            .padding(.horizontal, 14)
                            .frame(minHeight: 44)
                            .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(on ? Theme.text : Color.white.opacity(0.08)))
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(on ? [.isSelected] : [])
                }
            }
            .animation(.easeOut(duration: 0.15), value: current)
        }
        .modifier(SettingsRowBackground())
    }
}

/// Valeur mesurée ou fixe, à droite, en chasse fixe.
private struct SettingsValueRow: View {
    let title: String
    let note: String
    let value: String
    var warn = false

    var body: some View {
        HStack(spacing: 12) {
            SettingsLabel(title: title, note: note)
            Text(verbatim: value)
                .font(Theme.mono(13))
                .foregroundStyle(warn ? Theme.warnText : Theme.text2)
                .lineLimit(1)
                .fixedSize()
        }
        .modifier(SettingsRowBackground())
        .accessibilityElement(children: .combine)
    }
}

/// Ce que l'appareil sait faire : le titre, la valeur en dessous (elle peut
/// être longue), et une note.
private struct SettingsFactRow: View {
    let title: String
    let value: String
    let note: String
    var warn = false

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
            Text(verbatim: value)
                .font(Theme.mono(13))
                .foregroundStyle(warn ? Theme.warnText : Theme.text2)
                .fixedSize(horizontal: false, vertical: true)
            Text(note)
                .font(Theme.sans(12))
                .foregroundStyle(Theme.muted)
                .lineSpacing(2)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .modifier(SettingsRowBackground())
        .accessibilityElement(children: .combine)
    }
}

// MARK: - Ajouter un compte

/// « Ajouter un compte » (docs/link-ui.md, « Comptes KyberNow ») : le relais
/// d'abord, `share.kybernow.app` proposé, « Un autre relais… » pour un relais
/// chez soi (l'adresse est lue par le cœur avant de continuer) ; puis la page
/// de connexion du relais, dans la fenêtre du système.
struct AddAccountSheet: View {
    let accounts: AccountStore
    @Environment(\.dismiss) private var dismiss
    @StateObject private var relay = AccountRelayModel()
    /// « Un autre relais… » : le champ de l'adresse.
    @State private var other = false
    @FocusState private var focused: Bool

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(alignment: .top) {
                    Text(other ? AccountText.otherRelayTitle : AccountText.add)
                        .font(Theme.display(34, 660)).tracking(-34 * 0.03)
                        .foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 12)
                    Button { dismiss() } label: {
                        Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
                    }
                    .accessibilityLabel(Text(StationText.cancel))
                }
                if other {
                    relayField
                    if let failure = relay.failure {
                        AddHostNotice(kind: .warn, text: failure)
                    }
                    Button(AccountText.continueLabel) { continueWithOther() }
                        .buttonStyle(PrimaryButton(height: 54, fontSize: 17))
                        .disabled(!relay.canContinue)
                        .opacity(relay.canContinue ? 1 : 0.45)
                    Button(StationText.cancel) {
                        other = false
                        relay.changed()
                    }
                    .buttonStyle(GlassButton(height: 48, fontSize: 16))
                    .frame(maxWidth: .infinity)
                } else {
                    Text(AccountText.signInHelpWindow).font(Theme.sans(15)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                    Button { start("") } label: {
                        Text(verbatim: Accounts.defaultRelay).frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PrimaryButton(height: 54, fontSize: 17))
                    Button(AccountText.otherRelay) {
                        other = true
                        focused = true
                    }
                    .buttonStyle(GlassButton(height: 48, fontSize: 16))
                    .frame(maxWidth: .infinity)
                }
            }
            .padding(24)
            .frame(maxWidth: 560, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .scrollDismissesKeyboard(.interactively)
        .background(Theme.background)
        .environment(\.addHostWide, DeviceDisplay.isPad)
        .onChange(of: relay.text) { _, _ in relay.changed() }
    }

    /// « Relais », exemple grisé, sans correction ni majuscule automatiques.
    private var relayField: some View {
        let wide = DeviceDisplay.isPad
        return VStack(alignment: .leading, spacing: wide ? 8 : 6) {
            Text(AccountText.relayField).font(Theme.sans(wide ? 14 : 13)).foregroundStyle(Theme.muted)
            TextField(text: $relay.text, prompt: Text(verbatim: AccountText.relayPlaceholder).foregroundStyle(Theme.ghost)) { Text(AccountText.relayField) }
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .submitLabel(.go)
                .onSubmit { continueWithOther() }
                .focused($focused)
                .font(Theme.mono(wide ? 16 : 15))
                .foregroundStyle(Theme.text)
                .tint(Theme.accent)
                .padding(.horizontal, wide ? 18 : 16)
                .frame(height: wide ? 52 : 50)
                .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.surface))
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(focused ? Theme.accent : Theme.line2))
            Text(AccountText.relayHelp).font(Theme.sans(wide ? 13 : 12)).foregroundStyle(Theme.faint)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func continueWithOther() {
        guard relay.canContinue, let address = relay.validate() else { return }
        start(address)
    }

    /// La fiche se ferme, puis la page du relais s'ouvre (la fenêtre du
    /// système se pose sur celle des réglages).
    private func start(_ address: String) {
        dismiss()
        let accounts = accounts
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.45) { accounts.signIn(relay: address) }
    }
}
