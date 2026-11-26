// Accueil au premier lancement et incitation au compte KyberNow, sur iPhone
// et iPad (docs/link-ui.md, « Accueil et compte KyberNow » ; maquettes :
// https://claude.ai/artifact/K6tgZaZC8dqeBxGnTBenru, « iPhone »). Le modèle et
// les textes sont communs aux apps Apple (apps/macos/KyberNow/Welcome.swift) ;
// ici, les écrans au doigt, avec les composants de l'accueil (HomeParts.swift) :
//
// - l'accueil en trois écrans, par-dessus tout, après le lancement ;
// - le bouton du compte en haut de l'accueil (iPhone : l'icône seule) ;
// - la carte du compte en fin de rangée ;
// - le bandeau après un appairage, sous les boutons de l'ordinateur ;
// - « Hors de chez vous ? ».

import KyberKit
import SwiftUI

// MARK: - L'accueil en trois écrans

struct WelcomeCover: View {
    @ObservedObject var welcome: WelcomeStore
    @ObservedObject var accounts: AccountStore
    let app: AppModel
    @Environment(\.horizontalSizeClass) private var sizeClass

    private var wide: Bool { DeviceDisplay.isPad && sizeClass == .regular }

    var body: some View {
        ZStack {
            AmbientBackground().ignoresSafeArea()
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    HStack(spacing: 8) { Mark(size: wide ? 26 : 22); Wordmark(size: wide ? 20 : 17) }
                    Spacer()
                    Button(welcome.page == .computer ? WelcomeText.later : WelcomeText.skip) { welcome.finish() }
                        .font(Theme.sans(wide ? 16 : 15, .medium))
                        .foregroundStyle(Theme.muted)
                        .lineLimit(1)
                }
                .frame(height: 44)
                ScrollView(.vertical) {
                    Group {
                        switch welcome.page {
                        case .intro: intro
                        case .account: account
                        case .computer: computer
                        }
                    }
                    .id(welcome.page)
                    .transition(.opacity)
                    .frame(maxWidth: wide ? 760 : .infinity, alignment: .leading)
                    .padding(.top, wide ? 64 : 14)
                }
                .scrollIndicators(.hidden)
                .scrollBounceBehavior(.basedOnSize)
                progress.frame(maxWidth: .infinity).padding(.top, 12)
            }
            .padding(.horizontal, wide ? 72 : 22)
            .padding(.bottom, 12)
        }
        .animation(Theme.easeOut, value: welcome.page)
    }

    private var progress: some View {
        HStack(spacing: 8) {
            ForEach(WelcomeStore.Page.allCases, id: \.self) { p in
                Capsule().fill(p == welcome.page ? Theme.accent : Theme.line2)
                    .frame(width: p == welcome.page ? 26 : 10, height: 4)
            }
        }
        .accessibilityElement()
        .accessibilityLabel(Text(verbatim: WelcomeText.step(welcome.page.rawValue)))
    }

    private func eyebrow(_ n: Int) -> some View {
        Text(WelcomeText.step(n).uppercased())
            .font(Theme.mono(wide ? 12 : 11, .medium)).tracking(2)
            .foregroundStyle(Theme.accent)
    }

    private func title(_ text: String) -> some View {
        Text(text.uppercased())
            .font(Theme.display(wide ? 54 : 30, 760)).tracking(wide ? -1 : -0.5)
            .foregroundStyle(Theme.text)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityAddTraits(.isHeader)
    }

    private func paragraph(_ text: String) -> some View {
        Text(text).font(Theme.sans(wide ? 18 : 16)).foregroundStyle(Theme.text2).lineSpacing(3)
            .fixedSize(horizontal: false, vertical: true)
    }

    /// Le bouton principal : celui de « Se connecter » (HomeCTAStyle), sur une ligne.
    private func primary(_ label: String, busy: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Text(label).lineLimit(1).minimumScaleFactor(0.85)
                Spacer(minLength: 0)
                if busy { HomeSpinner(size: 20) } else { Icon("arrow-right", size: 20, bold: true) }
            }
            .padding(.horizontal, 20)
            .frame(maxWidth: wide ? 420 : .infinity)
        }
        .buttonStyle(HomeCTAStyle(online: true, height: 56, fontSize: 17))
        .disabled(busy)
    }

    private func secondary(_ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(label).lineLimit(1).minimumScaleFactor(0.85)
                .font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text)
                .frame(maxWidth: wide ? 420 : .infinity, minHeight: 52)
                .glass(12)
                .contentShape(Rectangle())
        }
        .buttonStyle(HomePressStyle())
    }

    // MARK: Écran 1

    private var intro: some View {
        VStack(alignment: .leading, spacing: wide ? 24 : 18) {
            eyebrow(1)
            title(WelcomeText.title1)
            paragraph(WelcomeText.text1)
            primary(WelcomeText.next) { welcome.next() }.padding(.top, 12)
        }
    }

    // MARK: Écran 2

    private var account: some View {
        VStack(alignment: .leading, spacing: wide ? 18 : 12) {
            eyebrow(2)
            title(WelcomeText.title2)
            paragraph(WelcomeText.text2)
            let columns = wide ? [GridItem(.flexible(), spacing: 14), GridItem(.flexible(), spacing: 14)] : [GridItem(.flexible())]
            LazyVGrid(columns: columns, alignment: .leading, spacing: wide ? 14 : 8) {
                ForEach(WelcomeText.perks, id: \.label) { perk in
                    HStack(alignment: .top, spacing: 12) {
                        Icon(perk.icon, size: 24).foregroundStyle(Theme.accent)
                        VStack(alignment: .leading, spacing: 3) {
                            Text(perk.label.uppercased()).font(Theme.mono(10, .medium)).tracking(1.4).foregroundStyle(Theme.faint)
                            Text(perk.text).font(Theme.sans(wide ? 14 : 13.5)).foregroundStyle(Theme.text2).lineSpacing(2)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        Spacer(minLength: 0)
                    }
                    .padding(wide ? 13 : 11)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .glass(14)
                }
            }
            .padding(.vertical, 6)
            VStack(alignment: wide ? .leading : .center, spacing: 10) {
                primary(accounts.signingIn ? AccountText.signingIn : WelcomeText.connect, busy: accounts.signingIn) { welcome.connect() }
                secondary(WelcomeText.create) { welcome.createAccount() }
                Button(WelcomeText.withoutAccount) { welcome.next() }
                    .font(Theme.sans(15)).foregroundStyle(Theme.accent)
                    .lineLimit(1)
                    .padding(8)
            }
            .frame(maxWidth: .infinity, alignment: wide ? .leading : .center)
            Text(WelcomeText.help(welcome.relayLabel)).font(Theme.sans(12.5)).foregroundStyle(Theme.faint)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    // MARK: Écran 3

    private var computer: some View {
        VStack(alignment: .leading, spacing: wide ? 18 : 14) {
            eyebrow(3)
            title(WelcomeText.title3)
            paragraph(WelcomeText.text3)
            Theme.eyebrow(WelcomeText.inReach).padding(.top, 12)
            // Les ordinateurs vus sur le réseau : la carte « Sur le réseau » de l'accueil.
            ScrollView(.horizontal) {
                HStack(spacing: 12) {
                    ForEach(app.newOnNetwork) { found in
                        Button {
                            welcome.finish()
                            app.sheet = .addHost(nil)
                        } label: {
                            HomeFoundCard(name: found.name, m: .phone, wide: false)
                        }
                        .buttonStyle(HomePressStyle())
                    }
                }
            }
            .scrollIndicators(.hidden)
            .scrollClipDisabled()
            infoCard(icon: "share-network", label: WelcomeText.codeLabel, text: WelcomeText.codeText) {
                Button(ShareText.join) {
                    welcome.finish()
                    app.sheet = .join(nil)
                }
                .font(Theme.sans(15, .semibold)).foregroundStyle(Theme.accent).lineLimit(1)
            }
            infoCard(icon: "link-simple", label: WelcomeText.notInstalled, text: WelcomeText.notInstalledText) {
                Button { welcome.openStationDownload() } label: {
                    HStack(spacing: 6) { Text(verbatim: WelcomeText.siteLabel); Icon("arrow-right", size: 14, bold: true) }
                }
                .font(Theme.sans(15, .semibold)).foregroundStyle(Theme.accent).lineLimit(1)
            }
        }
    }

    private func infoCard<A: View>(icon: String, label: String, text: String, @ViewBuilder action: () -> A) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 7) {
                Icon(icon, size: 15).foregroundStyle(Theme.muted)
                Theme.eyebrow(label)
            }
            Text(text).font(Theme.sans(14)).foregroundStyle(Theme.text2).lineSpacing(2)
                .fixedSize(horizontal: false, vertical: true)
            action()
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glass(14)
    }
}

// MARK: - Le bouton du compte

/// En haut de l'accueil, à côté de « Réglages de l'app », sans compte :
/// iPad en pleine largeur, la pilule ; iPhone, l'icône seule (libellé pour l'accessibilité).
struct AccountTopButton: View {
    @ObservedObject var accounts: AccountStore
    let wide: Bool

    var body: some View {
        if accounts.accounts.isEmpty {
            if wide {
                Button { accounts.signIn() } label: {
                    HStack(spacing: 10) {
                        if accounts.signingIn { HomeSpinner(size: 18) } else { Icon("user", size: 18) }
                        Text(WelcomeText.connect).lineLimit(1)
                    }
                }
                .buttonStyle(HomePillStyle())
                .fixedSize()
                .disabled(accounts.signingIn)
            } else {
                Button { accounts.signIn() } label: {
                    if accounts.signingIn { HomeSpinner(size: 18) } else { Icon("user", size: 20) }
                }
                .buttonStyle(HomeCircleStyle())
                .disabled(accounts.signingIn)
                .accessibilityLabel(Text(WelcomeText.connect))
            }
        }
    }
}

// MARK: - La carte du compte

/// En dernier dans la rangée, avec la lueur de « Sur le réseau ».
struct HomeAccountCard: View {
    @ObservedObject var welcome: WelcomeStore
    @ObservedObject var accounts: AccountStore
    let m: HomeCardMetrics
    let wide: Bool
    @State private var breathe = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: m.radius, style: .continuous)
        let close = welcome.cardCloseIsFinal ? WelcomeText.neverAgain : WelcomeText.close
        VStack(alignment: .leading, spacing: wide ? 6 : 4) {
            HStack(spacing: 6) {
                Icon("globe-hemisphere-west", size: wide ? 14 : 13, bold: true)
                Text(WelcomeText.cardLabel.uppercased()).font(Theme.mono(wide ? 10 : 9.5, .medium)).tracking(0.8).lineLimit(1)
            }
            .foregroundStyle(Theme.accentLight)
            .padding(.trailing, 28)
            Text(WelcomeText.cardTitle).font(Theme.display(wide ? 19 : 15, 640)).foregroundStyle(Theme.text)
                .lineLimit(2).minimumScaleFactor(0.8).fixedSize(horizontal: false, vertical: true)
            if wide {
                Text(WelcomeText.cardText).font(Theme.sans(13)).foregroundStyle(Theme.text2).lineLimit(2)
            }
            Spacer(minLength: 0)
            HStack(spacing: 8) {
                Button { accounts.signIn() } label: { Text(WelcomeText.connect).lineLimit(1).padding(.horizontal, 12) }
                    .buttonStyle(HomeCTAStyle(online: true, height: 34, fontSize: wide ? 14 : 13))
                    .fixedSize()
                    .disabled(accounts.signingIn)
                Button { welcome.createAccount() } label: {
                    Text(WelcomeText.create).lineLimit(1).font(Theme.sans(wide ? 14 : 13, .semibold)).foregroundStyle(Theme.text)
                        .padding(.horizontal, 12).frame(height: 34).glass(9)
                }
                .buttonStyle(HomePressStyle())
                .fixedSize()
            }
        }
        .padding(wide ? 16 : 12)
        .frame(width: max(m.width * 2, wide ? 0 : 316), height: m.height, alignment: .leading)
        .background(shape.fill(.ultraThinMaterial).environment(\.colorScheme, .dark))
        .background(shape.fill(Theme.accentFill.opacity(0.14)))
        .clipShape(shape)
        .overlay(
            shape.strokeBorder(Theme.accent.opacity(breathe ? 0.45 : 0.75), lineWidth: 1)
                .shadow(color: Theme.accentFill.opacity(breathe ? 0.2 : 0.35), radius: breathe ? 10 : 18)
        )
        .overlay(alignment: .topTrailing) {
            Button { withAnimation(Theme.easeOut) { welcome.closeCard() } } label: {
                Icon("x", size: 14, bold: true).foregroundStyle(Theme.muted).frame(width: 44, height: 44).contentShape(Rectangle())
            }
            .accessibilityLabel(Text(close))
        }
        .onAppear {
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 1.6).delay(0.6).repeatForever(autoreverses: true)) { breathe = true }
        }
    }
}

// MARK: - Après un appairage

/// Sous les boutons de l'ordinateur ajouté (après le bandeau de la Station : un seul à la fois).
struct PairedBannerCard: View {
    @ObservedObject var welcome: WelcomeStore
    @ObservedObject var accounts: AccountStore
    let wide: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top, spacing: 12) {
                Icon("globe-hemisphere-west", size: 20).foregroundStyle(Theme.accentLight).frame(width: 22, height: 22)
                Text(WelcomeText.pairedText)
                    .font(Theme.sans(wide ? 14.5 : 13.5)).foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            HStack(spacing: 10) {
                Button { accounts.signIn() } label: { Text(WelcomeText.connect).lineLimit(1).padding(.horizontal, 18) }
                    .buttonStyle(HomeCTAStyle(online: true, height: 40, fontSize: 15))
                    .disabled(accounts.signingIn)
                Button { withAnimation(Theme.easeOut) { welcome.dismissPairedBanner() } } label: { Text(WelcomeText.later).lineLimit(1) }
                    .buttonStyle(HomePillStyle())
            }
        }
        .padding(14)
        .frame(maxWidth: wide ? 640 : .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Theme.accentFill.opacity(0.12)))
        .glass(16)
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Theme.accent.opacity(0.5)))
    }
}

// MARK: - Posé sur l'app

/// L'accueil (par-dessus tout, après le lancement), « Hors de chez vous ? »,
/// et l'accueil ouvert au lancement s'il le faut.
struct WelcomePresenter: ViewModifier {
    @ObservedObject var app: AppModel
    @ObservedObject var welcome: WelcomeStore

    func body(content: Content) -> some View {
        content
            .overlay {
                if welcome.showing, app.launched {
                    WelcomeCover(welcome: welcome, accounts: app.accounts, app: app)
                        .transition(.opacity)
                }
            }
            .animation(Theme.join, value: welcome.showing)
            .alert(WelcomeText.awayTitle, isPresented: Binding(get: { welcome.away != nil }, set: { if !$0 { welcome.away = nil } }),
                   presenting: welcome.away) { _ in
                Button(WelcomeText.connect) { welcome.connect() }
                Button(WelcomeText.create) { welcome.createAccount() }
                Button(WelcomeText.notNow, role: .cancel) {}
            } message: { host in
                Text(WelcomeText.awayText(host.name))
            }
            .onChange(of: app.launched, initial: true) { _, launched in
                if launched { welcome.showIfNeeded() }
            }
    }
}
