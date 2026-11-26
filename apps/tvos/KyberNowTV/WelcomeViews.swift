// Accueil au premier lancement et incitation au compte KyberNow, sur l'Apple
// TV (docs/link-ui.md, « Accueil et compte KyberNow » ; maquette
// « Téléviseur » de https://claude.ai/artifact/K6tgZaZC8dqeBxGnTBenru). Le
// modèle et les textes sont communs aux apps Apple
// (apps/macos/KyberNow/Welcome.swift). Sur un téléviseur, pas de navigateur :
// l'écran 2 montre à droite le QR code et le code de la connexion par code
// (« Pas encore de compte ? Créez-le sur cette page. »), et « Continuer sans
// compte » est un bouton ; l'écran 3 dit l'adresse du site sans l'ouvrir.

import KyberKit
import SwiftUI

struct WelcomeTVView: View {
    @EnvironmentObject private var app: AppModel
    @ObservedObject var welcome: WelcomeStore
    @FocusState private var focus: Target?

    enum Target: Hashable { case skip, next, withoutAccount, join, newCode }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            HStack(spacing: 20) {
                Mark(size: 34)
                Wordmark(size: 28)
                Spacer(minLength: 0)
                Button { welcome.finish() } label: {
                    Text(welcome.page == .computer ? WelcomeText.later : WelcomeText.skip)
                        .font(Theme.sans(22, .medium)).lineLimit(1)
                        .padding(.horizontal, 26).frame(height: 60)
                }
                .buttonStyle(GlassButton(radius: 30))
                .focused($focus, equals: .skip)
            }
            .frame(width: 1920 - 192, height: 60)
            .padding(.leading, 96).padding(.top, 56)
            .focusSection()

            Group {
                switch welcome.page {
                case .intro: intro
                case .account: account
                case .computer: computer
                }
            }
            .id(welcome.page)
            .transition(.opacity)
            .padding(.leading, 96).padding(.top, 190)

            HStack(spacing: 10) {
                ForEach(WelcomeStore.Page.allCases, id: \.self) { p in
                    Capsule().fill(p == welcome.page ? Theme.accent : Theme.line2)
                        .frame(width: p == welcome.page ? 40 : 14, height: 6)
                }
            }
            .padding(.leading, 96).padding(.top, 1080 - 80)
            .accessibilityHidden(true)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, .next)
        .onExitCommand { welcome.finish() }
        .animation(Theme.join, value: welcome.page)
    }

    private func eyebrow(_ n: Int) -> some View {
        Text(WelcomeText.step(n).uppercased())
            .font(Theme.mono(16, .medium)).tracking(2.4)
            .foregroundStyle(Theme.accent)
    }

    private func title(_ text: String) -> some View {
        Text(text.uppercased())
            .font(Theme.display(64, 760)).tracking(-1)
            .foregroundStyle(Theme.text)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityAddTraits(.isHeader)
    }

    private func nextButton(_ label: String, focus target: Target) -> some View {
        Button { welcome.next() } label: {
            HStack(spacing: 16) { Text(label).lineLimit(1); Icon("arrow-right", size: 28, bold: true) }
        }
        .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
        .focused($focus, equals: target)
    }

    // MARK: Écran 1

    private var intro: some View {
        VStack(alignment: .leading, spacing: 30) {
            eyebrow(1)
            title(WelcomeText.title1).frame(width: 1100, alignment: .leading)
            Text(WelcomeText.text1).font(Theme.sans(28)).foregroundStyle(Theme.text2).lineSpacing(6)
                .frame(width: 1000, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
            nextButton(WelcomeText.next, focus: .next).padding(.top, 20)
        }
        .padding(.top, 80)
    }

    // MARK: Écran 2

    private var account: some View {
        HStack(alignment: .top, spacing: 80) {
            VStack(alignment: .leading, spacing: 28) {
                eyebrow(2)
                title(WelcomeText.title2).frame(width: 900, alignment: .leading)
                VStack(alignment: .leading, spacing: 20) {
                    ForEach(WelcomeText.perks, id: \.label) { perk in
                        HStack(alignment: .top, spacing: 18) {
                            Icon(perk.icon, size: 30).foregroundStyle(Theme.accent)
                            VStack(alignment: .leading, spacing: 4) {
                                Text(perk.label.uppercased()).font(Theme.mono(15, .medium)).tracking(1.6).foregroundStyle(Theme.faint)
                                Text(perk.text).font(Theme.sans(23)).foregroundStyle(Theme.text2)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }
                .frame(width: 900, alignment: .leading)
                // « Continuer sans compte » : un bouton, à la télécommande.
                Button { welcome.next() } label: {
                    Text(WelcomeText.withoutAccount).font(Theme.sans(24, .semibold)).lineLimit(1)
                        .padding(.horizontal, 30).frame(height: 70)
                }
                .buttonStyle(GlassButton(radius: 16))
                .focused($focus, equals: .next)
                .padding(.top, 10)
            }
            WelcomeCodeCard(accounts: app.accounts, demo: app.store.demo)
                .frame(width: 560)
        }
        .frame(width: 1920 - 192, alignment: .leading)
    }

    // MARK: Écran 3

    private var computer: some View {
        VStack(alignment: .leading, spacing: 28) {
            eyebrow(3)
            title(WelcomeText.title3).frame(width: 1300, alignment: .leading)
            Text(WelcomeText.text3).font(Theme.sans(26)).foregroundStyle(Theme.text2).lineSpacing(6)
                .frame(width: 1300, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
            Theme.eyebrow(WelcomeText.inReach).font(Theme.mono(18)).padding(.top, 16)
            HStack(alignment: .top, spacing: 24) {
                // Les ordinateurs vus sur le réseau (la carte « Sur le réseau » de l'accueil).
                ForEach(app.newOnNetwork.prefix(2)) { found in
                    Button {
                        welcome.finish()
                        app.screen = .addHost
                    } label: {
                        card(icon: "wifi-high", label: WelcomeText.onNetwork, title: found.name, action: WelcomeText.add, accent: true)
                    }
                    .buttonStyle(GlassButton(radius: 18, fill: Theme.accentFill.opacity(0.14)))
                }
                Button {
                    welcome.finish()
                    app.screen = .join(nil)
                } label: {
                    card(icon: "share-network", label: WelcomeText.codeLabel, title: nil, text: WelcomeText.codeText, action: ShareText.join)
                }
                .buttonStyle(GlassButton(radius: 18))
                .focused($focus, equals: .next)
                card(icon: "link-simple", label: WelcomeText.notInstalled, title: nil, text: WelcomeText.notInstalledText, action: WelcomeText.siteLabel)
                    .glass(18)
            }
            .focusSection()
        }
    }

    private func card(icon: String, label: String, title: String?, text: String? = nil, action: String, accent: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 10) {
                Icon(icon, size: 22)
                Text(label.uppercased()).font(Theme.mono(15, .medium)).tracking(1.4).lineLimit(1)
            }
            .foregroundStyle(accent ? Theme.accentLight : Theme.muted)
            if let title {
                Text(title).font(Theme.display(30, 640)).foregroundStyle(Theme.text).lineLimit(1).minimumScaleFactor(0.6)
            }
            if let text {
                Text(text).font(Theme.sans(21)).foregroundStyle(Theme.text2).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
            Text(action).font(Theme.sans(22, .semibold)).foregroundStyle(Theme.accentLight).lineLimit(1)
        }
        .padding(24)
        .frame(width: 400, height: 250, alignment: .topLeading)
    }
}

/// « CONNECTER UN COMPTE » : le QR code et le code de la connexion par code
/// (le modèle de l'écran du code, AccountCodeModel), « Pas encore de
/// compte ? Créez-le sur cette page. ». Connecté : l'accueil passe l'écran 3.
private struct WelcomeCodeCard: View {
    @StateObject private var model: AccountCodeModel
    /// Démonstration : aucun code demandé au relais.
    let demo: Bool

    init(accounts: AccountStore, demo: Bool) {
        _model = StateObject(wrappedValue: AccountCodeModel(accounts: accounts))
        self.demo = demo
    }

    private var code: AccountTVCode? {
        if case let .showing(code) = model.state { return code }
        return nil
    }

    var body: some View {
        VStack(spacing: 22) {
            Text(AccountText.signIn.uppercased()).font(Theme.mono(16, .medium)).tracking(2).foregroundStyle(Theme.muted)
            ZStack {
                RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Theme.text)
                if let code, let qr = Share.qrImage(for: code.urlComplete) {
                    Image(decorative: qr, scale: 1)
                        .interpolation(.none)
                        .resizable()
                        .aspectRatio(1, contentMode: .fit)
                        .padding(16)
                        .accessibilityLabel(Text(AccountText.qrLabel))
                } else {
                    ProgressView().tint(Theme.background)
                }
            }
            .frame(width: 280, height: 280)
            Text(verbatim: code?.code ?? "––––-––––")
                .font(Theme.mono(46, .semibold)).tracking(3)
                .foregroundStyle(code == nil ? Theme.faint : Theme.text)
                .lineLimit(1).minimumScaleFactor(0.6)
            VStack(spacing: 6) {
                Text(AccountText.stepScan)
                Text(AccountText.stepOpen(code?.urlLabel ?? "\(model.relayLabel)/tv"))
                Text(WelcomeText.codeNoAccount)
            }
            .font(Theme.sans(20)).foregroundStyle(Theme.text2).multilineTextAlignment(.center)
            switch model.state {
            case let .denied(text), let .failed(text):
                VStack(spacing: 12) {
                    Text(text).font(Theme.sans(19)).foregroundStyle(Theme.warnText).multilineTextAlignment(.center)
                    Button(AccountText.newCode) { model.restart() }
                        .buttonStyle(ChoiceButton(selected: false, height: 60, light: true))
                }
            case .showing:
                Text(AccountText.waiting(model.minutesLeft)).font(Theme.sans(18)).foregroundStyle(Theme.faint).multilineTextAlignment(.center)
            case .loading:
                EmptyView()
            }
        }
        .padding(34)
        .frame(maxWidth: .infinity)
        .glass(26)
        .task(id: model.attempt) { if !demo { await model.run() } }
    }
}

/// La carte du compte, en fin de rangée « Vos ordinateurs » : la carte
/// entière ouvre le code (« Connecter un compte ») ; un appui long propose
/// « Fermer » (puis « Ne plus afficher »).
struct TVAccountCard: View {
    @ObservedObject var welcome: WelcomeStore
    let width: CGFloat
    let connect: () -> Void

    var body: some View {
        let close = welcome.cardCloseIsFinal ? WelcomeText.neverAgain : WelcomeText.close
        Button(action: connect) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    Icon("globe-hemisphere-west", size: 20)
                    Text(WelcomeText.cardLabel.uppercased()).font(Theme.mono(14, .medium)).tracking(1.2).lineLimit(1).minimumScaleFactor(0.7)
                }
                .foregroundStyle(Theme.accentLight)
                Text(WelcomeText.cardTitle).font(Theme.display(23, 640)).foregroundStyle(Theme.text)
                    .lineLimit(3).minimumScaleFactor(0.7).fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
                Text(WelcomeText.connect).font(Theme.sans(20, .semibold)).foregroundStyle(Theme.accentLight).lineLimit(1).minimumScaleFactor(0.7)
            }
            .padding(20)
            .frame(width: width, height: 216, alignment: .leading)
            .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Theme.accent.opacity(0.7)))
        }
        .buttonStyle(GlassButton(radius: 18, fill: Theme.accentFill.opacity(0.16)))
        .contextMenu {
            Button(close) { withAnimation(Theme.join) { welcome.closeCard() } }
        }
        .accessibilityHint(Text(WelcomeText.cardText))
    }
}
