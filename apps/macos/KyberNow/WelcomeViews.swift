// Accueil au premier lancement et incitation au compte KyberNow, sur Mac
// (docs/link-ui.md, « Accueil et compte KyberNow » ; maquettes :
// https://claude.ai/artifact/K6tgZaZC8dqeBxGnTBenru). Le modèle et les textes
// sont communs aux apps Apple (Welcome.swift) ; ici, les écrans du Mac, avec
// les composants de l'accueil (boutons, cartes de la rangée, carte « Sur le
// réseau » et sa lueur) :
//
// - l'accueil en trois écrans, par-dessus la fenêtre de l'accueil ;
// - le bouton « Connecter un compte » en haut de l'accueil, sans compte ;
// - la carte du compte en fin de rangée « Vos autres ordinateurs » ;
// - le bandeau après un appairage, sous les boutons de l'ordinateur ;
// - la fenêtre « Hors de chez vous ? ».

import KyberKit
import SwiftUI

// MARK: - Posé sur l'accueil

/// L'accueil en trois écrans et la fenêtre « Hors de chez vous ? », posés
/// sur la fenêtre de l'accueil ; l'accueil s'ouvre au lancement s'il le faut.
struct WelcomeLayer: ViewModifier {
    @ObservedObject var welcome: WelcomeStore

    func body(content: Content) -> some View {
        content
            .overlay {
                if welcome.showing {
                    WelcomeScreens(welcome: welcome)
                        .transition(.opacity)
                }
            }
            .animation(.easeInOut(duration: 0.3), value: welcome.showing)
            .sheet(item: $welcome.away) { AwaySheet(host: $0, welcome: welcome) }
            .onAppear { welcome.showIfNeeded() }
    }
}

// MARK: - L'accueil en trois écrans

private struct WelcomeScreens: View {
    @EnvironmentObject var app: AppModel
    @ObservedObject var welcome: WelcomeStore
    @ObservedObject var accounts = AppModel.shared.accounts
    @State private var arrived = false

    var body: some View {
        ZStack(alignment: .topLeading) {
            Theme.background
            Signature(seed: "KyberNow")
            LinearGradient(stops: [.init(color: Theme.background.opacity(0.9), location: 0), .init(color: Theme.background.opacity(0.5), location: 0.45), .init(color: .clear, location: 0.75)],
                           startPoint: .leading, endPoint: .trailing)
                .allowsHitTesting(false)
            Group {
                switch welcome.page {
                case .intro: intro
                case .account: account
                case .computer: computer
                }
            }
            .id(welcome.page)
            .transition(.opacity)
            .padding(.leading, 72)
            .padding(.trailing, 72)
            .padding(.top, 104)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            progress
                .padding(.leading, 72).padding(.bottom, 40)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading)
            header
        }
        .ignoresSafeArea()
        .animation(.easeInOut(duration: 0.3), value: welcome.page)
        .onAppear { arrived = true }
    }

    /// Le symbole et le nom, à leur place de l'accueil ; « Passer » ou « Plus tard ».
    private var header: some View {
        HStack(spacing: 10) {
            Mark(size: 18)
            Wordmark(size: 14)
            Spacer()
            Button(welcome.page == .computer ? WelcomeText.later : WelcomeText.skip) { welcome.finish() }
                .buttonStyle(GhostButton(height: 32))
                .fixedSize()
                .keyboardShortcut(.cancelAction)
        }
        .padding(.leading, 92)
        .padding(.trailing, 18)
        .frame(height: 56)
    }

    /// Trois traits, celui de l'écran en cours allongé.
    private var progress: some View {
        HStack(spacing: 8) {
            ForEach(WelcomeStore.Page.allCases, id: \.self) { p in
                Capsule().fill(p == welcome.page ? Theme.accent : Theme.line2)
                    .frame(width: p == welcome.page ? 26 : 10, height: 4)
            }
        }
        .animation(Theme.easeOut, value: welcome.page)
        .accessibilityElement()
        .accessibilityLabel(Text(verbatim: WelcomeText.step(welcome.page.rawValue)))
    }

    private func eyebrow(_ n: Int) -> some View {
        Text(WelcomeText.step(n).uppercased())
            .font(Theme.mono(12, .medium)).tracking(2)
            .foregroundStyle(Theme.accent)
    }

    private func title(_ text: String, size: CGFloat) -> some View {
        Text(text.uppercased())
            .font(Theme.display(size, .bold)).tracking(-size * 0.02)
            .lineSpacing(-size * 0.12)
            .foregroundStyle(Theme.text)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityAddTraits(.isHeader)
    }

    private func paragraph(_ text: String) -> some View {
        Text(text)
            .font(Theme.sans(18)).foregroundStyle(Theme.text2)
            .lineSpacing(4)
            .fixedSize(horizontal: false, vertical: true)
    }

    // MARK: Écran 1

    private var intro: some View {
        VStack(alignment: .leading, spacing: 24) {
            eyebrow(1)
            title(WelcomeText.title1, size: 54).frame(maxWidth: 620, alignment: .leading)
            paragraph(WelcomeText.text1).frame(maxWidth: 580, alignment: .leading)
            Button { welcome.next() } label: {
                HStack(spacing: 14) { Text(WelcomeText.next).lineLimit(1); Icon("arrow-right", size: 18, bold: true) }
            }
            .buttonStyle(PrimaryButton(height: 56))
            .fixedSize()
            .keyboardShortcut(.defaultAction)
            .padding(.top, 8)
        }
        .padding(.top, 66)
    }

    // MARK: Écran 2

    private var account: some View {
        let signingIn = accounts.signingIn
        return VStack(alignment: .leading, spacing: 18) {
            eyebrow(2)
            title(WelcomeText.title2, size: 52).frame(maxWidth: 760, alignment: .leading)
            paragraph(WelcomeText.text2).frame(maxWidth: 760, alignment: .leading)
            HStack(alignment: .top, spacing: 16) {
                ForEach(WelcomeText.perks, id: \.label) { perk in
                    VStack(alignment: .leading, spacing: 12) {
                        Icon(perk.icon, size: 24).foregroundStyle(Theme.accent)
                        Text(perk.label.uppercased()).font(Theme.mono(11, .medium)).tracking(1.4).foregroundStyle(Theme.faint)
                            .lineLimit(1).minimumScaleFactor(0.8)
                        Text(perk.text).font(Theme.sans(15)).foregroundStyle(Theme.text2).lineSpacing(3)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .padding(18)
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                    .glass(14)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: 1100)
            .padding(.top, 22)
            HStack(spacing: 14) {
                Button { welcome.connect() } label: {
                    HStack(spacing: 14) {
                        if signingIn { ProgressView().controlSize(.small) }
                        Text(signingIn ? AccountText.signingIn : WelcomeText.connect).lineLimit(1)
                        if !signingIn { Icon("arrow-right", size: 18, bold: true) }
                    }
                }
                .buttonStyle(PrimaryButton(height: 56, lit: signingIn))
                .fixedSize()
                .disabled(signingIn)
                .keyboardShortcut(.defaultAction)
                Button { welcome.createAccount() } label: { Text(WelcomeText.create).lineLimit(1) }
                    .buttonStyle(GhostButton(height: 56))
                    .fixedSize()
                Button { welcome.next() } label: {
                    Text(WelcomeText.withoutAccount).font(Theme.sans(16)).foregroundStyle(Theme.accent).lineLimit(1)
                }
                .buttonStyle(.plain)
                .fixedSize()
                .padding(.leading, 10)
            }
            .padding(.top, 26)
            Text(WelcomeText.help(welcome.relayLabel))
                .font(Theme.sans(13)).foregroundStyle(Theme.faint)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    // MARK: Écran 3

    private var computer: some View {
        VStack(alignment: .leading, spacing: 18) {
            eyebrow(3)
            title(WelcomeText.title3, size: 52).frame(maxWidth: 760, alignment: .leading)
            paragraph(WelcomeText.text3).frame(maxWidth: 640, alignment: .leading)
            Text(WelcomeText.inReach.uppercased())
                .font(Theme.mono(13, .medium)).tracking(2.4).foregroundStyle(Theme.muted)
                .padding(.top, 34)
            InReachRow(welcome: welcome, discovery: app.discovery)
        }
    }
}

/// « ORDINATEURS À PORTÉE » : les ordinateurs vus sur le réseau (la carte
/// « Sur le réseau » de l'accueil), « Un code », « Pas encore installé ».
private struct InReachRow: View {
    @EnvironmentObject var app: AppModel
    @ObservedObject var welcome: WelcomeStore
    @ObservedObject var discovery: Discovery
    @State private var resolved: [String: Candidate] = [:]

    /// Annoncés, joignables, pas encore ajoutés (comme la rangée de l'accueil).
    private var newcomers: [(found: Discovery.Found, candidate: Candidate)] {
        discovery.found.compactMap { f in
            guard let c = resolved[f.id] else { return nil }
            let known = app.store.hosts.contains { h in
                Fingerprint.same(h.consoleFingerprint, f.fingerprint) || (h.address == c.address && h.consolePort == c.port)
            }
            return known ? nil : (f, c)
        }
    }

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(alignment: .top, spacing: 16) {
                ForEach(newcomers, id: \.found.id) { item in
                    FoundCard(found: item.found, candidate: item.candidate) {
                        welcome.finish()
                        app.add(link: item.candidate)
                    }
                    .transition(.scale(scale: 0.96).combined(with: .opacity))
                }
                InfoCard(icon: "share-network", label: WelcomeText.codeLabel, text: WelcomeText.codeText) {
                    Button(ShareText.join) {
                        welcome.finish()
                        app.join()
                    }
                    .buttonStyle(SignalButton(height: 32))
                    .fixedSize()
                }
                InfoCard(icon: "desktop-tower", label: WelcomeText.notInstalled, text: WelcomeText.notInstalledText) {
                    Button { welcome.openStationDownload() } label: {
                        HStack(spacing: 6) {
                            Text(verbatim: WelcomeText.siteLabel).font(Theme.sans(14, .semibold))
                            Icon("arrow-right", size: 13, bold: true)
                        }
                        .foregroundStyle(Theme.accent)
                    }
                    .buttonStyle(.plain)
                    .fixedSize()
                }
            }
            .padding(.vertical, 18)
            .padding(.horizontal, 2)
            .animation(Theme.easeOut, value: newcomers.map(\.found.id))
        }
        .padding(.vertical, -18)
        .scrollClipDisabled()
        .task(id: discovery.found.map(\.id)) {
            for found in discovery.found where resolved[found.id] == nil {
                if let c = await Discovery.resolve(found) { resolved[found.id] = c }
            }
        }
    }
}

/// Une carte de l'écran 3 : étiquette, texte, une action.
private struct InfoCard<Action: View>: View {
    let icon: String
    let label: String
    let text: String
    @ViewBuilder let action: () -> Action

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 7) {
                Icon(icon, size: 14)
                Theme.eyebrow(label, color: Theme.muted)
            }
            .foregroundStyle(Theme.muted)
            Text(text).font(Theme.sans(14)).foregroundStyle(Theme.text2).lineSpacing(2)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
            action()
        }
        .padding(16)
        .frame(width: cardSize.width + 40, height: cardSize.height, alignment: .topLeading)
        .glass(14)
    }
}

// MARK: - Le bouton du compte

/// En haut de l'accueil, à côté de « Réglages de l'app », sans compte.
struct AccountHeaderButton: View {
    @ObservedObject var accounts: AccountStore

    var body: some View {
        if accounts.accounts.isEmpty {
            Button { accounts.signIn() } label: {
                HStack(spacing: 8) {
                    if accounts.signingIn { ProgressView().controlSize(.small) } else { Icon("user", size: 16) }
                    Text(WelcomeText.connect).lineLimit(1)
                }
            }
            .buttonStyle(PrimaryButton(height: 32, lit: accounts.signingIn))
            .fixedSize()
            .disabled(accounts.signingIn)
        }
    }
}

// MARK: - La carte du compte

/// En dernier dans « Vos autres ordinateurs », avec la lueur de « Sur le
/// réseau » : « Retrouvez vos ordinateurs partout ». La croix « Fermer »
/// (revient 30 jours plus tard), puis « Ne plus afficher ».
struct AccountShelfCard: View {
    @ObservedObject var welcome: WelcomeStore
    @ObservedObject var accounts = AppModel.shared.accounts
    @State private var ignite = 0
    @State private var breathe = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let close = welcome.cardCloseIsFinal ? WelcomeText.neverAgain : WelcomeText.close
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 7) {
                HStack(spacing: 7) {
                    Icon("globe-hemisphere-west", size: 14)
                    Theme.eyebrow(WelcomeText.cardLabel, color: Theme.accentLight)
                }
                .foregroundStyle(Theme.accentLight)
                Text(WelcomeText.cardTitle).font(Theme.display(19, .semibold)).tracking(-0.3).foregroundStyle(Theme.text).lineLimit(1)
                    .minimumScaleFactor(0.8)
                Text(WelcomeText.cardText).font(Theme.sans(13.5)).foregroundStyle(Theme.text2).lineLimit(2)
            }
            .padding(.trailing, 26)
            Spacer(minLength: 0)
            HStack(spacing: 10) {
                Button { accounts.signIn() } label: { Text(WelcomeText.connect).lineLimit(1) }
                    .buttonStyle(PrimaryButton(height: 32, lit: accounts.signingIn))
                    .fixedSize()
                    .disabled(accounts.signingIn)
                Button { welcome.createAccount() } label: { Text(WelcomeText.create).lineLimit(1) }
                    .buttonStyle(GhostButton(height: 32))
                    .fixedSize()
            }
        }
        .padding(16)
        .frame(width: 2 * cardSize.width - 18, height: cardSize.height, alignment: .topLeading)
        .background(Theme.accentFill.opacity(0.12), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.accent.opacity(breathe ? 0.5 : 0.75)))
        .shadow(color: Theme.accentFill.opacity(breathe ? 0.2 : 0.35), radius: breathe ? 10 : 18)
        .overlay(alignment: .topTrailing) {
            Button { withAnimation(Theme.easeOut) { welcome.closeCard() } } label: {
                Icon("x", size: 13, bold: true).foregroundStyle(Theme.muted)
                    .frame(width: 28, height: 28).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .padding(8)
            .help(close)
            .accessibilityLabel(Text(close))
        }
        .neonIgnition(trigger: ignite, delay: 0.6)
        .task {
            ignite += 1
            if !reduceMotion {
                try? await Task.sleep(for: .seconds(1.8))
                Theme.breathe { breathe = $0 }
            }
        }
    }
}

// MARK: - Après un appairage

/// Sous les boutons de l'ordinateur ajouté (un bandeau au plus : la mise à
/// jour de la Station passe avant).
struct PairedBanner: View {
    @ObservedObject var welcome: WelcomeStore
    @ObservedObject var accounts = AppModel.shared.accounts

    var body: some View {
        HStack(spacing: 14) {
            Icon("globe-hemisphere-west", size: 20).foregroundStyle(Theme.accentLight)
                .frame(width: 24)
            Text(WelcomeText.pairedText)
                .font(Theme.sans(14)).foregroundStyle(Theme.text2)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button { accounts.signIn() } label: { Text(WelcomeText.connect).lineLimit(1) }
                .buttonStyle(PrimaryButton(height: 36, lit: accounts.signingIn))
                .fixedSize()
                .disabled(accounts.signingIn)
            Button(WelcomeText.later) { withAnimation(Theme.easeOut) { welcome.dismissPairedBanner() } }
                .buttonStyle(GhostButton(height: 36))
                .fixedSize()
        }
        .padding(.horizontal, 16).padding(.vertical, 12)
        .background(Theme.accentFill.opacity(0.12), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .glass(12)
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Theme.accent.opacity(0.6)))
    }
}

// MARK: - Hors de chez vous

private struct AwaySheet: View {
    let host: Host
    @ObservedObject var welcome: WelcomeStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 26) {
            HStack(alignment: .top, spacing: 18) {
                Icon("globe-hemisphere-west", size: 30).foregroundStyle(Theme.accent)
                    .frame(width: 64, height: 64)
                    .background(Theme.accentSoft, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                VStack(alignment: .leading, spacing: 10) {
                    Text(WelcomeText.awayTitle).font(Theme.display(28, .bold)).foregroundStyle(Theme.text)
                    Text(WelcomeText.awayText(host.name)).font(Theme.sans(15)).foregroundStyle(Theme.text2).lineSpacing(3)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            HStack(spacing: 12) {
                Spacer()
                Button(WelcomeText.notNow) { dismiss() }
                    .buttonStyle(.plain)
                    .font(Theme.sans(15)).foregroundStyle(Theme.text2)
                    .fixedSize()
                    .keyboardShortcut(.cancelAction)
                Button { dismiss(); welcome.createAccount() } label: { Text(WelcomeText.create).lineLimit(1) }
                    .buttonStyle(GhostButton(height: 44))
                    .fixedSize()
                Button {
                    dismiss()
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { welcome.connect() }
                } label: { Text(WelcomeText.connect).lineLimit(1) }
                    .buttonStyle(PrimaryButton())
                    .fixedSize()
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(34)
        .frame(width: 640)
        .background(Theme.background)
        .preferredColorScheme(.dark)
    }
}

extension AppModel {
    /// « Réessayer » sur un ordinateur appairé : le sondage, puis, s'il ne
    /// répond toujours pas sur ce réseau sans route par le relais, « Hors de
    /// chez vous ? » (sans compte, une fois par jour et par ordinateur).
    func retry(_ host: Host) async {
        await store.retry(host)
        if welcome.offerAway(host, status: store.status[host.id]) {
            route = .home
            showWindow()
        }
    }

    /// Un ordinateur vient d'être appairé : il passe en avant, avec son
    /// bandeau (sans compte, une fois par ordinateur).
    func paired(_ host: Host) {
        welcome.paired(host)
        if welcome.pairedPending == host.id { selectedID = host.id }
    }
}
