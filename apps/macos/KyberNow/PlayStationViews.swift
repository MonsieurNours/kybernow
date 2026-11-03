// Consoles PlayStation à l'accueil et leur association (textes :
// docs/link-ui.md, « Consoles PlayStation ») : la console choisie (état,
// nom, modèle, jeu en cours, bouton principal, puce « HDR »), sa carte dans
// la rangée des ordinateurs, la carte « Sur le réseau » d'une console pas
// encore associée, et la feuille de l'association en deux étapes (compte
// PSN dans une fenêtre web, une fois pour toutes ; puis le code à 8 chiffres).

import KyberKit
import SwiftUI
import WebKit

// MARK: - La console choisie

struct ConsoleHero: View {
    @EnvironmentObject var app: AppModel
    let console: PlayStationConsole
    @ObservedObject var store: PlayStationStore
    let arrived: Bool
    let size: CGSize
    @StateObject private var screen = HDRScreenWatch()

    private var nameSize: CGFloat { min(104, size.width * 0.072, size.height * 0.095) }
    private var status: PlayStationStore.Status { store.status(console) }
    private var paired: Bool { store.paired.contains(console.id) }

    private func rise(_ i: Int) -> some ViewModifier { Rise(arrived: arrived, delay: 0.15 + Double(i) * 0.1) }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            StatusPill(online: status.online, text: status.text)
                .id(status)
                .modifier(rise(0))
            VStack(alignment: .leading, spacing: 10) {
                Text(verbatim: console.name)
                    .font(Theme.display(nameSize, .bold))
                    .tracking(-nameSize * 0.045)
                    .lineLimit(2)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                    .frame(maxWidth: size.width * 0.56, alignment: .leading)
                Text(verbatim: console.model).font(Theme.sans(16)).foregroundStyle(Theme.muted).lineLimit(1)
            }
            .modifier(rise(1))
            if let game = store.live(console)?.runningApp {
                HStack(spacing: 10) {
                    Icon("game-controller", size: 18).foregroundStyle(Theme.accent)
                    Text(verbatim: game).font(Theme.sans(15)).foregroundStyle(Theme.text2)
                }
                .modifier(rise(2))
            }
            actions
                .padding(.top, 10)
                .modifier(rise(3))
        }
        .frame(maxWidth: 820, alignment: .leading)
    }

    @ViewBuilder private var actions: some View {
        HStack(spacing: 14) {
            if !paired {
                Button { store.pairing = PairTarget(console: console, live: store.live(console)) } label: {
                    HStack(spacing: 10) { Icon("link-simple", size: 18, bold: true); Text(PSText.pair) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
            } else if let session = store.sessions[console.id] {
                Button { session.bringToFront() } label: {
                    HStack(spacing: 12) { Text(LinkText.showSession); Icon("arrow-right", size: 18, bold: true) }
                }
                .buttonStyle(PrimaryButton(height: 56, lit: true))
                .keyboardShortcut(.defaultAction)
            } else if store.waking.contains(console.id) {
                Button {} label: {
                    HStack(spacing: 10) { ProgressView().controlSize(.small); Text(PSText.waking) }
                }
                .buttonStyle(GhostButton(height: 56))
                .disabled(true)
            } else if status == .on {
                Button { store.play(console, app: app) } label: {
                    HStack(spacing: 12) { Icon("game-controller", size: 18, bold: true); Text(PSText.play) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
            } else if status == .standby {
                Button { store.wakeAndPlay(console, app: app) } label: {
                    HStack(spacing: 10) { Icon("power", size: 18, bold: true); Text(PSText.wakeAndPlay) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
            } else if status == .offline {
                Button { Task { await store.searchOnce() } } label: {
                    HStack(spacing: 10) { Icon("arrows-clockwise", size: 18); Text("Réessayer") }
                }
                .buttonStyle(GhostButton(height: 56))
            } else {
                Button {} label: { Text(PSText.play) }
                    .buttonStyle(PrimaryButton(height: 56))
                    .disabled(true)
            }
            // La puce « HDR » : mêmes règles que pour un ordinateur (écran HDR ; PS5).
            if paired, console.hasHDR, screen.hdr {
                HDRChipButton(on: console.hdr, available: true, help: HDRText.help) {
                    store.setHDR(!console.hdr, for: console)
                }
            }
        }
    }
}

// MARK: - Dans la rangée

/// Une console associée : son nom et son état.
struct ConsoleCard: View {
    @EnvironmentObject var app: AppModel
    let console: PlayStationConsole
    @ObservedObject var store: PlayStationStore
    let select: () -> Void
    @State private var hovering = false

    var body: some View {
        let status = store.status(console)
        Button(action: select) {
            VStack(alignment: .leading, spacing: 0) {
                Signature(seed: console.name)
                    .frame(height: 86)
                    .clipped()
                VStack(alignment: .leading, spacing: 4) {
                    Text(verbatim: console.name).font(Theme.sans(15, .semibold)).foregroundStyle(status.online == false ? Theme.muted : Theme.text).lineLimit(1)
                    HStack(spacing: 7) {
                        Circle()
                            .fill(status.online == true ? Theme.ok : .clear)
                            .overlay(Circle().strokeBorder(status.online == true ? .clear : Theme.faint))
                            .frame(width: 6, height: 6)
                        Text(status.text).font(Theme.sans(12)).foregroundStyle(Theme.muted).lineLimit(1)
                    }
                }
                .padding(.horizontal, 14).padding(.vertical, 11)
            }
            .frame(width: cardSize.width, height: cardSize.height, alignment: .top)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .glass(14)
            .offset(y: hovering ? -3 : 0)
            .shadow(color: .black.opacity(hovering ? 0.5 : 0), radius: 18, y: 12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { h in withAnimation(Theme.easeOut) { hovering = h } }
        .contextMenu {
            Button(String(localized: "Retirer")) { store.remove(console) }
        }
    }
}

/// Console trouvée sur le réseau, pas encore associée.
struct ConsoleFoundCard: View {
    let found: PlayStation.Found
    let pair: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 7) {
                    Icon("wifi-high", size: 14)
                    Theme.eyebrow(String(localized: "Sur le réseau"), color: Theme.accentLight)
                }
                .foregroundStyle(Theme.accentLight)
                Text(verbatim: found.name).font(Theme.display(22, .semibold)).tracking(-0.4).foregroundStyle(Theme.text).lineLimit(1)
                Text(verbatim: found.model).font(Theme.sans(12)).foregroundStyle(Theme.muted)
            }
            Spacer(minLength: 0)
            Button(PSText.pair, action: pair)
                .buttonStyle(SignalButton(height: 32))
        }
        .padding(16)
        .frame(width: cardSize.width, height: cardSize.height, alignment: .topLeading)
        .background(Theme.accentFill.opacity(0.12), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.accent.opacity(0.6)))
    }
}

// MARK: - Association

struct PairConsoleSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    @ObservedObject private var store = AppModel.shared.playstation
    let target: PairTarget

    private enum Step { case account, code }
    @State private var step: Step = .account
    @State private var web = false
    @State private var byHand = false
    @State private var manualID = ""
    @State private var code = ""
    @State private var busy = false
    @State private var error: String?
    @FocusState private var codeFocused: Bool

    /// Identifiant saisi à la main : 8 octets en base64 (12 caractères).
    private var manualValid: Bool {
        Data(base64Encoded: manualID.trimmingCharacters(in: .whitespaces))?.count == 8
    }
    private var digits: String { code.filter(\.isNumber) }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(verbatim: target.found.name).font(Theme.display(30)).foregroundStyle(Theme.text)
                    Text(verbatim: target.found.model).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                }
                Spacer()
                Button { dismiss() } label: { Icon("x", size: 15, bold: true) }
                    .buttonStyle(GhostButton(height: 34))
                    .keyboardShortcut(.cancelAction)
                    .accessibilityLabel(StationText.cancel)
            }
            switch step {
            case .account: account
            case .code: codeStep
            }
            if let error { Notice(kind: .warn, text: error) }
        }
        .padding(.horizontal, 36).padding(.vertical, 32)
        .frame(width: web ? 720 : 640)
        .background {
            ZStack {
                Theme.background
                RadialGradient(colors: [Theme.accentFill.opacity(0.22), .clear], center: .topTrailing, startRadius: 0, endRadius: 520)
            }
        }
        .preferredColorScheme(.dark)
        .onAppear {
            // Le compte se lit une fois pour toutes.
            step = store.accountID == nil ? .account : .code
            codeFocused = step == .code
        }
    }

    @ViewBuilder private var account: some View {
        Text(PSText.signInHelp).font(Theme.sans(14)).foregroundStyle(Theme.text2)
            .fixedSize(horizontal: false, vertical: true)
        if web {
            PSNWebView { url in signedIn(url) }
                .frame(height: 520)
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .overlay { if busy { ProgressView() } }
        } else {
            Button { error = nil; web = true } label: {
                HStack(spacing: 10) { Icon("user", size: 18, bold: true); Text(PSText.signIn) }
            }
            .buttonStyle(PrimaryButton(height: 48))
        }
        Button(PSText.byHand) { byHand.toggle() }
            .buttonStyle(.plain)
            .font(Theme.sans(13))
            .foregroundStyle(Theme.accentLight)
        if byHand {
            HStack(alignment: .bottom, spacing: 12) {
                LabeledField(label: PSText.accountField, text: $manualID, prompt: "AbCdEfGhIjk=", mono: true)
                    .onSubmit(useManualID)
                Button(String(localized: "Continuer"), action: useManualID)
                    .buttonStyle(PrimaryButton(height: 44))
                    .disabled(!manualValid)
            }
        }
    }

    @ViewBuilder private var codeStep: some View {
        Text(PSText.codeHelp).font(Theme.sans(14)).foregroundStyle(Theme.text2)
            .fixedSize(horizontal: false, vertical: true)
        TextField(text: $code, prompt: Text(verbatim: "1234 5678").foregroundColor(Theme.ghost)) { EmptyView() }
            .textFieldStyle(.plain)
            .font(Theme.mono(34, .medium))
            .tracking(4)
            .foregroundStyle(Theme.accentLight)
            .multilineTextAlignment(.center)
            .focused($codeFocused)
            .onSubmit(pair)
            .padding(.horizontal, 14)
            .frame(height: 78)
            .background(Theme.bar, in: RoundedRectangle(cornerRadius: 14))
            .overlay(RoundedRectangle(cornerRadius: 14).stroke(codeFocused ? Theme.accentLine : Theme.line2))
            .disabled(busy)
            .accessibilityLabel(PSText.codeField)
            .help(PSText.codeField)
        HStack {
            Spacer()
            Button(action: pair) {
                HStack(spacing: 10) {
                    if busy { ProgressView().controlSize(.small) }
                    Text(PSText.pair)
                }
            }
            .buttonStyle(PrimaryButton(lit: busy))
            .disabled(digits.count != 8 || busy)
            .keyboardShortcut(.defaultAction)
        }
    }

    private func useManualID() {
        guard manualValid else { return }
        store.accountID = manualID.trimmingCharacters(in: .whitespaces)
        error = nil
        step = .code
        codeFocused = true
    }

    /// Sony a rendu l'adresse de retour : l'identifiant en est tiré (deux requêtes).
    private func signedIn(_ url: URL) {
        guard !busy else { return }
        busy = true
        Task {
            let result = await Task.detached { PlayStation.account(redirect: url) }.value
            busy = false
            web = false
            switch result {
            case let .success(id):
                store.accountID = id
                error = nil
                step = .code
                codeFocused = true
            case let .failure(e):
                KyberLog.app.notice("PlayStation : identifiant du compte non lu (\(e.detail, privacy: .public))")
                error = PSText.accountUnread
                byHand = true
            }
        }
    }

    private func pair() {
        guard digits.count == 8, !busy, let account = store.accountID, let pin = UInt32(digits) else { return }
        busy = true
        error = nil
        let found = target.found
        Task {
            let result = await Task.detached { PlayStation.regist(address: found.address, target: found.target, account: account, pin: pin) }.value
            busy = false
            switch result {
            case let .success(registered):
                let console = store.paired(found, registered: registered)
                app.selectedID = console.id
                dismiss()
            case let .failure(e):
                error = PSText.regist(e)
            }
        }
    }
}

/// La page de connexion de Sony. Son adresse de retour n'est jamais
/// chargée : elle est rendue à `onRedirect`. Rien n'est gardé (ni cookies
/// ni cache) une fois la feuille fermée.
struct PSNWebView: NSViewRepresentable {
    let onRedirect: (URL) -> Void

    /// Safari de bureau : la page de Sony refuse certains navigateurs intégrés.
    private static let userAgent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15"

    func makeCoordinator() -> Coordinator { Coordinator(onRedirect: onRedirect) }

    func makeNSView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        let view = WKWebView(frame: .zero, configuration: config)
        view.customUserAgent = Self.userAgent
        view.navigationDelegate = context.coordinator
        if let url = PlayStation.loginURL { view.load(URLRequest(url: url)) }
        return view
    }

    func updateNSView(_ view: WKWebView, context: Context) {}

    final class Coordinator: NSObject, WKNavigationDelegate {
        let onRedirect: (URL) -> Void
        private var done = false

        init(onRedirect: @escaping (URL) -> Void) { self.onRedirect = onRedirect }

        private func caught(_ url: URL?) -> Bool {
            guard let url, PlayStation.isRedirect(url) else { return false }
            if !done {
                done = true
                onRedirect(url)
            }
            return true
        }

        func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void) {
            decisionHandler(caught(action.request.url) ? .cancel : .allow)
        }

        func webView(_ webView: WKWebView, didReceiveServerRedirectForProvisionalNavigation navigation: WKNavigation!) {
            if caught(webView.url) { webView.stopLoading() }
        }

        func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
            let url = (error as NSError).userInfo[NSURLErrorFailingURLErrorKey] as? URL
            if !caught(url) {
                KyberLog.app.notice("PlayStation : page de Sony pas chargée (\(error.localizedDescription, privacy: .public))")
            }
        }
    }
}
