// Comptes KyberNow sur l'Apple TV (docs/comptes.md, lot 3 ; textes :
// docs/link-ui.md, « Comptes KyberNow » ; maquette, rangée « Apps Link ») :
// l'écran du code (approuvé depuis le téléphone), « Un autre relais… », et
// les comptes de l'Apple TV (Réglages de l'app › Comptes). Le modèle est
// commun aux apps Apple (apps/macos/KyberNow/Accounts.swift) ; l'Apple TV
// n'a pas de navigateur : « Gérer le compte » montre le QR code de la page
// du compte, à ouvrir sur le téléphone.

import KyberKit
import SwiftUI

// MARK: - Le code

/// « Se connecter à un compte KyberNow » : trois étapes, le code en chasse
/// fixe, le QR code de la page avec le code ; une demande toutes les
/// `interval` secondes tant que l'écran est là (`.task`, annulée en le
/// quittant) ; échu, un nouveau code de lui-même ; refusé sur le téléphone,
/// « Nouveau code ».
struct AccountCodeView: View {
    @EnvironmentObject private var app: AppModel
    @StateObject private var model: AccountCodeModel
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, newCode, otherRelay }

    init(accounts: AccountStore, relay: String) {
        _model = StateObject(wrappedValue: AccountCodeModel(accounts: accounts, relay: relay))
    }

    private var code: AccountTVCode? {
        if case let .showing(code) = model.state { return code }
        return nil
    }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            Button { leave() } label: {
                HStack(spacing: 10) {
                    Icon("arrow-left", size: 26, bold: true)
                    Text("Retour").font(Theme.sans(22, .medium))
                }
                .padding(.leading, 18).padding(.trailing, 26)
                .frame(height: 60)
            }
            .buttonStyle(GlassButton(radius: 30))
            .focused($focus, equals: .back)
            .padding(.leading, 96).padding(.top, 56)

            // À gauche : le titre, les étapes, le code, l'attente, le relais.
            VStack(alignment: .leading, spacing: 30) {
                Text(AccountText.codeTitle)
                    .font(Theme.display(64, 660)).tracking(-64 * 0.035)
                    .foregroundStyle(Theme.text)
                    .fixedSize(horizontal: false, vertical: true)
                VStack(alignment: .leading, spacing: 14) {
                    step(1, AccountText.stepScan)
                    step(2, AccountText.stepOpen(code?.urlLabel ?? "\(model.relayLabel)/tv"))
                    step(3, AccountText.stepConnect)
                    // Pas de navigateur sur l'Apple TV : la page du code crée aussi le compte.
                    Text(WelcomeText.codeNoAccount).font(Theme.sans(23)).foregroundStyle(Theme.muted)
                }
                Text(verbatim: code?.code ?? "––––-––––")
                    .font(Theme.mono(104, .semibold))
                    .tracking(6)
                    .foregroundStyle(code == nil ? Theme.faint : Theme.text)
                    .lineLimit(1)
                    .minimumScaleFactor(0.6)
                    .accessibilityLabel(Text(verbatim: code.map { $0.code.map(String.init).joined(separator: " ") } ?? ""))
                status
                relayLine
            }
            .focusSection()
            .frame(width: 1040, alignment: .leading)
            .padding(.leading, 96).padding(.top, 170)

            // À droite : le QR code de la page avec le code.
            qrCard
                .frame(width: 520)
                .padding(.leading, 1920 - 96 - 520).padding(.top, 250)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, .back)
        .onExitCommand { leave() }
        .task(id: model.attempt) { await model.run() }
        .onAppear {
            // Connecté : l'accueil, où arrivent les ordinateurs du compte (le rappel ne retient que l'app).
            model.onDone = { [app] in app.screen = .home }
        }
        .onDisappear { model.onDone = nil }
        .onChange(of: model.state) { _, state in
            // Refusé ou en échec : le focus sur « Nouveau code ».
            switch state {
            case .denied, .failed: focus = .newCode
            default: break
            }
        }
    }

    private func step(_ n: Int, _ text: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Text(verbatim: "\(n).").font(Theme.sans(25, .semibold)).foregroundStyle(Theme.accent)
            Text(text).font(Theme.sans(25)).foregroundStyle(Theme.text2).lineSpacing(6)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    /// L'attente (« Le code change dans {n} min. »), ou le refus avec « Nouveau code ».
    @ViewBuilder private var status: some View {
        switch model.state {
        case .loading:
            ProgressView()
                .frame(height: 44)
        case .showing:
            HStack(spacing: 16) {
                ProgressView()
                Text(AccountText.waiting(model.minutesLeft)).font(Theme.sans(23)).foregroundStyle(Theme.muted)
            }
        case let .denied(text), let .failed(text):
            HStack(spacing: 24) {
                HStack(spacing: 12) {
                    Icon("warning", size: 26).foregroundStyle(Theme.warn)
                    Text(text).font(Theme.sans(23)).foregroundStyle(Theme.warnText)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Button(AccountText.newCode) { model.restart() }
                    .buttonStyle(ChoiceButton(selected: false, height: 64, light: true))
                    .focused($focus, equals: .newCode)
            }
        }
    }

    /// « Relais : {relais} » et « Un autre relais… ».
    private var relayLine: some View {
        HStack(spacing: 24) {
            Text(AccountText.relay(model.relayLabel)).font(Theme.sans(22)).foregroundStyle(Theme.muted)
            Button(AccountText.otherRelay) { app.screen = .accountRelay }
                .buttonStyle(ChoiceButton(selected: false, height: 60, light: true))
                .focused($focus, equals: .otherRelay)
        }
        .focusSection()
    }

    private var qrCard: some View {
        VStack(spacing: 18) {
            ZStack {
                RoundedRectangle(cornerRadius: 24, style: .continuous).fill(Theme.text)
                if let code, let qr = Share.qrImage(for: code.urlComplete) {
                    Image(decorative: qr, scale: 1)
                        .interpolation(.none)
                        .resizable()
                        .aspectRatio(1, contentMode: .fit)
                        .padding(18)
                        .accessibilityLabel(Text(AccountText.qrLabel))
                } else {
                    ProgressView().tint(Theme.background)
                }
            }
            .frame(width: 380, height: 380)
            Text(AccountText.qrLabel).font(Theme.sans(19)).foregroundStyle(Theme.muted)
        }
        .padding(36)
        .glass(30)
    }

    private func leave() {
        app.screen = app.accountReturn
    }
}

// MARK: - Un autre relais

/// « Un autre relais » : l'adresse du relais du compte, lue par le cœur avant
/// de continuer ; « Continuer » ouvre le code sur ce relais (ses refus, « Ce
/// relais n'a pas de comptes KyberNow. », « Le relais ne répond pas. »,
/// s'y disent avec « Nouveau code »).
struct AccountRelayView: View {
    @EnvironmentObject private var app: AppModel
    @StateObject private var relay = AccountRelayModel()
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, field, go, cancel }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            Button { cancel() } label: {
                HStack(spacing: 10) {
                    Icon("arrow-left", size: 26, bold: true)
                    Text("Retour").font(Theme.sans(22, .medium))
                }
                .padding(.leading, 18).padding(.trailing, 26)
                .frame(height: 60)
            }
            .buttonStyle(GlassButton(radius: 30))
            .focused($focus, equals: .back)
            .padding(.leading, 96).padding(.top, 56)

            VStack(alignment: .leading, spacing: 26) {
                Text(AccountText.otherRelayTitle).font(Theme.display(64, 660)).tracking(-64 * 0.035)
                VStack(alignment: .leading, spacing: 10) {
                    Text(AccountText.relayField).font(Theme.sans(19)).foregroundStyle(Theme.muted)
                    TextField(text: $relay.text, prompt: Text(verbatim: AccountText.relayPlaceholder)) { Text(AccountText.relayField) }
                        .font(Theme.mono(30))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                        .focused($focus, equals: .field)
                        .onSubmit { go() }
                        .frame(width: 900)
                    Text(AccountText.relayHelp).font(Theme.sans(21)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if let failure = relay.failure {
                    HStack(spacing: 12) {
                        Icon("warning", size: 26).foregroundStyle(Theme.warn)
                        Text(failure).font(Theme.sans(22)).foregroundStyle(Theme.warnText)
                    }
                }
                HStack(spacing: 20) {
                    Button(AccountText.continueLabel) { go() }
                        .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
                        .disabled(!relay.canContinue)
                        .focused($focus, equals: .go)
                    Button(StationText.cancel) { cancel() }
                        .buttonStyle(ChoiceButton(selected: false, height: 80, light: true))
                        .focused($focus, equals: .cancel)
                }
                .focusSection()
            }
            .focusSection()
            .frame(width: 1100, alignment: .leading)
            .padding(.leading, 96).padding(.top, 190)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, .field)
        .onExitCommand { cancel() }
        .onChange(of: relay.text) { _, _ in relay.changed() }
    }

    private func go() {
        guard relay.canContinue, let address = relay.validate() else { return }
        app.screen = .accountCode(address)
    }

    /// Le code sur share.kybernow.app.
    private func cancel() {
        app.screen = .accountCode("")
    }
}

// MARK: - Les comptes (Réglages de l'app)

/// Réglages de l'app › « Comptes » : chaque compte (nom, e-mail, relais),
/// « Gérer le compte » (le QR code de la page du compte, à ouvrir sur le
/// téléphone : créer, changer le mot de passe, supprimer s'y font) et « Se
/// déconnecter », confirmé ; « Ajouter un compte » ouvre le code.
struct AccountsView: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var accounts: AccountStore
    @FocusState private var focus: Target?
    /// « Gérer le compte » : le QR code de sa page, à droite.
    @State private var managing: String?
    /// « Se déconnecter », à confirmer.
    @State private var signingOut: KyberAccount?

    enum Target: Hashable { case back, add, sync, manage(String), signOut(String) }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            Button { app.screen = .appSettings } label: {
                HStack(spacing: 10) {
                    Icon("arrow-left", size: 26, bold: true)
                    Text("Retour").font(Theme.sans(22, .medium))
                }
                .padding(.leading, 18).padding(.trailing, 26)
                .frame(height: 60)
            }
            .buttonStyle(GlassButton(radius: 30))
            .focused($focus, equals: .back)
            .padding(.leading, 96).padding(.top, 56)

            VStack(alignment: .leading, spacing: 16) {
                Text(AccountText.section).font(Theme.display(64, 660)).tracking(-64 * 0.035)
                Text(AccountText.sectionHelp).font(Theme.sans(22)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.bottom, 14)
                if accounts.accounts.isEmpty {
                    Text(AccountText.none).font(Theme.sans(24, .semibold)).foregroundStyle(Theme.text)
                }
                ForEach(accounts.accounts) { a in
                    row(a)
                }
                // Réglages synchronisés : seulement avec un compte connecté.
                if !accounts.accounts.isEmpty { syncRow }
                Button { app.showAccountCode(from: .accounts) } label: {
                    HStack(spacing: 14) {
                        Icon("plus", size: 26, bold: true)
                        Text(accounts.accounts.isEmpty ? AccountText.signIn : AccountText.add)
                    }
                }
                .buttonStyle(PrimaryButton(height: 72, fontSize: 24))
                .focused($focus, equals: .add)
                .padding(.top, 10)
                Text(AccountText.signInHelpCode).font(Theme.sans(19)).foregroundStyle(Theme.muted)
            }
            .focusSection()
            .frame(width: 1080, alignment: .leading)
            .padding(.leading, 96).padding(.top, 170)

            if let key = managing, let a = accounts.account(key) {
                manageCard(a)
                    .frame(width: 520)
                    .padding(.leading, 1920 - 96 - 520).padding(.top, 250)
                    .transition(.opacity)
            }
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, accounts.accounts.isEmpty ? .add : .manage(accounts.accounts[0].key))
        .onExitCommand { app.screen = .appSettings }
        .animation(Theme.join, value: managing)
        .confirmationDialog(AccountText.signOutQuestion(signingOut?.displayName ?? ""),
                            isPresented: Binding(get: { signingOut != nil }, set: { if !$0 { signingOut = nil } }), titleVisibility: .visible) {
            Button(AccountText.signOut, role: .destructive) {
                if let a = signingOut {
                    if managing == a.key { managing = nil }
                    accounts.signOut(a)
                }
                signingOut = nil
            }
            Button(StationText.cancel, role: .cancel) { signingOut = nil }
        }
    }

    private func row(_ a: KyberAccount) -> some View {
        HStack(spacing: 20) {
            VStack(alignment: .leading, spacing: 4) {
                Text(verbatim: a.displayName).font(Theme.sans(24, .semibold)).foregroundStyle(Theme.text)
                if !a.name.isEmpty, !a.email.isEmpty {
                    Text(verbatim: a.email).font(Theme.sans(19)).foregroundStyle(Theme.text2)
                }
                Text(verbatim: a.relayLabel).font(Theme.mono(17)).foregroundStyle(Theme.muted)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Button(AccountText.manage) { managing = managing == a.key ? nil : a.key }
                .buttonStyle(ChoiceButton(selected: managing == a.key, height: 60, light: true))
                .focused($focus, equals: .manage(a.key))
            Button(AccountText.signOut) { signingOut = a }
                .buttonStyle(ChoiceButton(selected: false, height: 60, light: true))
                .focused($focus, equals: .signOut(a.key))
        }
        .padding(.horizontal, 24).padding(.vertical, 14)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
        .focusSection()
    }

    /// « Synchroniser les réglages des ordinateurs » (allumé par défaut, gardé sur cette Apple TV).
    private var syncRow: some View {
        Button { accounts.syncSettings.toggle() } label: {
            HStack(spacing: 24) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(verbatim: Accounts.syncSettingsTitle).font(Theme.sans(24, .semibold)).foregroundStyle(Theme.text)
                    Text(verbatim: Accounts.syncSettingsHelp).font(Theme.sans(19)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Toggle2(on: accounts.syncSettings)
            }
            .padding(.horizontal, 24).padding(.vertical, 14)
            .frame(minHeight: 88)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .sync)
        .accessibilityValue(accounts.syncSettings ? Text("Activé") : Text("Désactivé"))
    }

    /// « Gérer le compte » : la page du compte, par le QR code et son adresse.
    private func manageCard(_ a: KyberAccount) -> some View {
        VStack(spacing: 18) {
            Text(AccountText.manage).font(Theme.sans(24, .semibold)).foregroundStyle(Theme.text)
            ZStack {
                RoundedRectangle(cornerRadius: 24, style: .continuous).fill(Theme.text)
                if let qr = Share.qrImage(for: a.console) {
                    Image(decorative: qr, scale: 1)
                        .interpolation(.none)
                        .resizable()
                        .aspectRatio(1, contentMode: .fit)
                        .padding(18)
                }
            }
            .frame(width: 340, height: 340)
            Text(verbatim: a.console.replacingOccurrences(of: "https://", with: ""))
                .font(Theme.mono(19)).foregroundStyle(Theme.muted)
                .lineLimit(1).minimumScaleFactor(0.6)
        }
        .padding(36)
        .glass(30)
    }
}
