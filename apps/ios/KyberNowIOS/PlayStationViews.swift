// Consoles PlayStation dans la fiche d'ajout de l'iPhone et de l'iPad
// (docs/link-ui.md, « Consoles PlayStation ») : la voie « PlayStation »
// (comptes PlayStation Network, consoles sur ce réseau, envoi entre
// appareils), les consoles de « Sur le réseau », et la feuille de
// l'association (compte, connexion Sony dans une vue web, code de la
// console). Logique et textes de l'app Mac (PlayStationViews.swift,
// `PSText`) ; présentation de la fiche d'ajout de l'iPhone (AddHostPanes).

import KyberKit
import SwiftUI
import WebKit

extension AppModel {
    /// Une console à associer depuis la fiche d'ajout : la fiche se ferme,
    /// puis la feuille de l'association s'ouvre (une feuille à la fois).
    func pair(fromAddSheet target: PairTarget) {
        sheet = nil
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { self.playstation.pairing = target }
    }
}

// MARK: - Éléments

/// Une ligne des listes de comptes et de consoles (même verre que les
/// voies de l'ajout) ; allumée : une console à associer.
struct PSRow<Trailing: View>: View {
    let title: String
    var detail: String?
    var lit = false
    @ViewBuilder let trailing: () -> Trailing
    @Environment(\.addHostWide) private var wide

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 14) {
                texts
                Spacer(minLength: 8)
                HStack(spacing: 8) { trailing() }
            }
            VStack(alignment: .leading, spacing: 12) {
                texts
                HStack(spacing: 8) { trailing() }
            }
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
        .frame(maxWidth: .infinity, minHeight: wide ? 80 : 72, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(lit ? Theme.accentFill.opacity(0.14) : Color.white.opacity(0.05)))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(lit ? Theme.accent.opacity(0.6) : Color.white.opacity(0.08)))
    }

    private var texts: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(verbatim: title).font(Theme.sans(wide ? 18 : 16, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
            if let detail {
                Text(verbatim: detail).font(Theme.sans(wide ? 14 : 13)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// Encadré d'information (« Ce compte est déjà là. ») : le signal, en veille.
struct PSInfoNotice: View {
    let text: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Icon("check", size: 20).foregroundStyle(Theme.accent)
            Text(text).font(Theme.sans(14)).foregroundStyle(Theme.text2)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 14).padding(.vertical, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Theme.accentFill.opacity(0.12)))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.accent.opacity(0.35)))
    }
}

/// Titre d'une partie du volet (« Comptes PlayStation Network »…).
private struct PSSectionTitle: View {
    let text: String
    @Environment(\.addHostWide) private var wide

    var body: some View {
        Text(text).font(Theme.display(wide ? 22 : 20, 640)).foregroundStyle(Theme.text)
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityAddTraits(.isHeader)
    }
}

/// Une console trouvée (voie « PlayStation » et « Sur le réseau ») :
/// « {nom} · PS5 », son état ; pas associée : « Associer » ; associée :
/// « Associée : {pseudos} » et, s'il reste un compte, « Associer avec un
/// autre compte ».
struct PSFoundRow: View {
    let found: PlayStation.Found
    @ObservedObject var store: PlayStationStore
    let pair: (PairTarget) -> Void

    var body: some View {
        let kept = store.console(hostID: found.hostID)
        let accounts = kept.map { store.pairedAccounts($0) } ?? []
        let state = found.ready ? PSText.on : PSText.standby
        PSRow(title: "\(found.name) · \(found.model)",
              detail: accounts.isEmpty ? "\(state) · \(found.address)" : "\(state) · \(PSText.pairedWith(accounts.map { store.name($0) }))",
              lit: accounts.isEmpty) {
            if accounts.isEmpty {
                Button { pair(target(kept)) } label: { Text(PSText.pair).padding(.horizontal, 20) }
                    .buttonStyle(PrimaryButton(height: 44, fontSize: 15))
                    .fixedSize()
            } else if !store.unpairedAccounts(hostID: found.hostID).isEmpty {
                Button(PSText.pairAnother) { pair(target(kept)) }
                    .buttonStyle(GlassButton(height: 44, fontSize: 14))
            }
        }
    }

    private func target(_ kept: PlayStationConsole?) -> PairTarget {
        kept.map { PairTarget(console: $0, live: found) } ?? PairTarget(found: found)
    }
}

// MARK: - La voie « PlayStation »

/// Les comptes PlayStation Network, puis les consoles sur ce réseau, puis
/// l'envoi entre appareils. Les consoles sont cherchées tant que le volet
/// est à l'écran.
struct AddHostPlayStationPane: View {
    @EnvironmentObject private var app: AppModel
    @Environment(\.addHostWide) private var wide

    var body: some View {
        PlayStationPaneContent(app: app, store: app.playstation, wide: wide)
    }
}

private struct PlayStationPaneContent: View {
    let app: AppModel
    @ObservedObject var store: PlayStationStore
    let wide: Bool
    @State private var adding = false
    @State private var already = false
    @State private var renaming: PSNAccount?
    @State private var newName = ""
    @State private var removing: PSNAccount?

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 20 : 14) {
            AddHostHeader(eyebrow: LinkText.addTitle, compactTitle: PSText.tab, wideTitle: PSText.tab) {
                AddHostWaitingDots(size: 7)
            }
            PSSectionTitle(text: PSText.accountsTitle)
            if store.accounts.isEmpty {
                Text(PSText.noAccount).font(Theme.sans(wide ? 16 : 14.5)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            ForEach(store.accounts) { a in
                PSRow(title: store.name(a), detail: PSText.consoleCount(store.consoles(of: a).count)) {
                    Button(PSText.rename) { newName = store.name(a); renaming = a }
                        .buttonStyle(GlassButton(height: 40, fontSize: 14))
                    Button(PSText.remove) { removing = a }
                        .buttonStyle(GlassButton(height: 40, fontSize: 14))
                }
            }
            if already { PSInfoNotice(text: PSText.alreadyHere) }
            Button { already = false; adding = true } label: {
                HStack(spacing: 8) { Icon("plus", size: 16, bold: true); Text(PSText.addAccount) }
            }
            .buttonStyle(GlassButton(height: 48, fontSize: 15))

            PSSectionTitle(text: PSText.consolesTitle).padding(.top, 12)
            let list = store.onNetwork
            if list.isEmpty {
                Text(PSText.noConsole).font(Theme.sans(wide ? 16 : 14.5)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            ForEach(list, id: \.hostID) { f in
                PSFoundRow(found: f, store: store) { app.pair(fromAddSheet: $0) }
            }

            PSHandoffSection(store: store)
                .padding(.top, 12)
        }
        .frame(maxWidth: wide ? 640 : .infinity, alignment: .leading)
        // Les consoles sont cherchées tant que le volet est à l'écran (pas en démonstration).
        .task {
            guard !app.store.demo else { return }
            while !Task.isCancelled {
                if store.sessions.isEmpty { await store.searchOnce() }
                try? await Task.sleep(for: .seconds(3))
            }
        }
        .sheet(isPresented: $adding) {
            PairConsoleSheet(target: .newAccount, store: store) { existed in already = existed }
                .environmentObject(app)
        }
        .alert(PSText.rename, isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } }), presenting: renaming) { a in
            TextField(PSText.nameField, text: $newName)
            Button(PSText.rename) { store.rename(a, to: newName); renaming = nil }
            Button(PSText.cancel, role: .cancel) { renaming = nil }
        }
        .confirmationDialog(removing.map { PSText.removeQuestion(store.name($0), consoles: store.consoles(of: $0).count) } ?? "",
                            isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
                            titleVisibility: .visible, presenting: removing) { a in
            Button(PSText.remove, role: .destructive) { store.removeAccount(a); removing = nil }
            Button(PSText.cancel, role: .cancel) { removing = nil }
        } message: { _ in
            Text(PSText.removeHelp)
        }
    }
}

// MARK: - Association

/// La feuille de l'association : le choix du compte (« Avec quel compte ? »),
/// le compte (connexion Sony, ou l'identifiant à la main), puis le code à
/// 8 chiffres de la console. Sans console (« Ajouter un compte ») : le compte
/// seul, gardé, et la feuille se ferme.
struct PairConsoleSheet: View {
    let target: PairTarget
    @ObservedObject var store: PlayStationStore
    /// « Ajouter un compte » : le compte est gardé, `true` s'il était déjà là.
    var accountAdded: ((Bool) -> Void)?
    @EnvironmentObject private var app: AppModel
    @Environment(\.dismiss) private var dismiss

    private enum Step { case choose, account, code }
    @State private var step: Step = .account
    @State private var web = false
    @State private var byHand = false
    @State private var manualID = ""
    @State private var manualName = ""
    /// Le compte avec lequel associer (étape du code).
    @State private var account: PSNAccount?
    /// Le compte lu était déjà gardé : « Ce compte est déjà là. »
    @State private var already = false
    @State private var code = ""
    @State private var busy = false
    /// La page de Sony est arrivée (avant : l'attente).
    @State private var webLoaded = false
    @State private var error: String?
    @FocusState private var codeFocused: Bool

    /// Identifiant saisi à la main : 8 octets en base64 (12 caractères).
    private var manualValid: Bool {
        Data(base64Encoded: manualID.trimmingCharacters(in: .whitespaces))?.count == 8
    }
    private var digits: String { code.filter(\.isNumber) }
    /// Comptes qu'on peut encore associer à cette console.
    private var free: [PSNAccount] { target.found.map { store.unpairedAccounts(hostID: $0.hostID) } ?? [] }

    var body: some View {
        VStack(spacing: 0) {
            header
                .padding(.horizontal, 24).padding(.top, 20).padding(.bottom, 8)
            if web, step == .account {
                // La page de Sony prend toute la place qui reste.
                VStack(alignment: .leading, spacing: 12) {
                    Text(PSText.signInHelp).font(Theme.sans(14)).foregroundStyle(Theme.text2)
                        .fixedSize(horizontal: false, vertical: true)
                    PSNWebView(onLoaded: { webLoaded = true }) { url in signedIn(url) }
                        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.line2))
                        .overlay { if busy || !webLoaded { ProgressView().tint(Theme.accent).controlSize(.large) } }
                    Button(PSText.byHand) { web = false; byHand = true }
                        .buttonStyle(.plain)
                        .font(Theme.sans(14)).foregroundStyle(Theme.accentLight)
                        .frame(minHeight: 44)
                }
                .padding(.horizontal, 24).padding(.bottom, 16)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        switch step {
                        case .choose: choose
                        case .account: accountStep
                        case .code: codeStep
                        }
                        if let error { AddHostNotice(kind: .warn, text: error) }
                    }
                    .padding(.horizontal, 24).padding(.top, 8).padding(.bottom, 32)
                    .frame(maxWidth: 640, alignment: .leading)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .scrollDismissesKeyboard(.interactively)
                .scrollBounceBehavior(.basedOnSize)
            }
        }
        .background(AmbientBackground().ignoresSafeArea())
        .preferredColorScheme(.dark)
        .presentationDragIndicator(.visible)
        .interactiveDismissDisabled(busy)
        .onAppear {
            // Deux comptes ou plus pas encore associés : le choix ; un seul : le code ; aucun : l'étape Compte.
            switch target.found == nil ? 0 : free.count {
            case 0: step = .account
            case 1: account = free[0]; step = .code
            default: step = .choose
            }
            if step == .code { DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { codeFocused = true } }
        }
    }

    private var header: some View {
        HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: 4) {
                if let found = target.found {
                    Text(verbatim: found.name).font(Theme.display(30, 660)).tracking(-0.9).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(verbatim: found.model).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                } else {
                    Text(PSText.addAccount).font(Theme.display(30, 660)).tracking(-0.9).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .accessibilityAddTraits(.isHeader)
            Spacer(minLength: 12)
            Button { dismiss() } label: {
                Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
            }
            .keyboardShortcut(.cancelAction)
            .accessibilityLabel(Text(PSText.cancel))
        }
    }

    // « Avec quel compte ? »
    @ViewBuilder private var choose: some View {
        Text(PSText.whichAccount).font(Theme.display(22, 640)).foregroundStyle(Theme.text)
        ForEach(free) { a in
            Button {
                account = a
                error = nil
                step = .code
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { codeFocused = true }
            } label: {
                PSRow(title: store.name(a), detail: PSText.consoleCount(store.consoles(of: a).count)) {
                    Icon("arrow-right", size: 16, bold: true).foregroundStyle(Theme.ghost)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(AddHostPressStyle())
        }
        Button { error = nil; step = .account } label: {
            HStack(spacing: 8) { Icon("plus", size: 16, bold: true); Text(PSText.addAccount) }
        }
        .buttonStyle(GlassButton(height: 48, fontSize: 15))
    }

    @ViewBuilder private var accountStep: some View {
        Text(PSText.signInHelp).font(Theme.sans(15)).foregroundStyle(Theme.text2)
            .lineSpacing(3)
            .fixedSize(horizontal: false, vertical: true)
        Button { error = nil; webLoaded = false; web = true } label: {
            HStack(spacing: 10) { Icon("user", size: 18, bold: true); Text(PSText.signIn) }
        }
        .addHostPrimary()
        Button(PSText.byHand) { byHand.toggle() }
            .buttonStyle(.plain)
            .font(Theme.sans(14)).foregroundStyle(Theme.accentLight)
            .frame(minHeight: 44)
        if byHand {
            AddHostField(label: PSText.accountField, text: $manualID, prompt: "AbCdEfGhIjk=", mono: true, onSubmit: useManualID)
            VStack(alignment: .leading, spacing: 6) {
                AddHostField(label: PSText.nameField, text: $manualName, submit: .continue, onSubmit: useManualID)
                Text(PSText.nameHelp).font(Theme.sans(13)).foregroundStyle(Theme.faint)
            }
            Button(String(localized: "Continuer"), action: useManualID)
                .addHostPrimary()
                .disabled(!manualValid)
        }
    }

    @ViewBuilder private var codeStep: some View {
        if let account {
            HStack(spacing: 8) {
                Icon("user", size: 16, bold: true).foregroundStyle(Theme.accent)
                Text(verbatim: PSText.accountLine(store.name(account))).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
            }
        }
        if already { PSInfoNotice(text: PSText.alreadyHere) }
        Text(PSText.codeHelp).font(Theme.sans(15)).foregroundStyle(Theme.text2)
            .lineSpacing(3)
            .fixedSize(horizontal: false, vertical: true)
        TextField(text: $code, prompt: Text(verbatim: "1234 5678").foregroundStyle(Theme.ghost)) { Text(PSText.codeField) }
            .keyboardType(.numberPad)
            .textContentType(.oneTimeCode)
            .font(Theme.mono(32, .medium))
            .tracking(4)
            .foregroundStyle(Theme.accentLight)
            .tint(Theme.accent)
            .multilineTextAlignment(.center)
            .focused($codeFocused)
            .onSubmit(pair)
            .onChange(of: code) { _, v in
                let d = String(v.filter(\.isNumber).prefix(8))
                if d != v.filter(\.isNumber) || v.count > 9 { code = d }
            }
            .padding(.horizontal, 14)
            .frame(height: 76)
            .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Theme.surface))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(codeFocused ? Theme.accent : Theme.line2))
            .disabled(busy)
            .accessibilityLabel(Text(PSText.codeField))
        Button(action: pair) {
            HStack(spacing: 10) {
                if busy { ProgressView().tint(.white) }
                Text(PSText.pair)
            }
        }
        .addHostPrimary()
        .disabled(digits.count != 8 || busy)
    }

    private func useManualID() {
        guard manualValid else { return }
        kept(manualID.trimmingCharacters(in: .whitespaces), name: manualName)
    }

    /// Un compte lu ou saisi : gardé tout de suite (même si l'association
    /// échoue ensuite), puis le code ; sans console, la feuille se ferme.
    private func kept(_ id: String, name: String?) {
        let (a, existed) = store.keep(accountID: id, name: name)
        guard target.found != nil else {
            accountAdded?(existed)
            dismiss()
            return
        }
        account = a
        already = existed
        error = nil
        step = .code
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { codeFocused = true }
    }

    /// Sony a rendu l'adresse de retour : l'identifiant et le pseudo en sont tirés.
    private func signedIn(_ url: URL) {
        guard !busy else { return }
        busy = true
        Task {
            let result = await Task.detached { PlayStation.account(redirect: url) }.value
            busy = false
            web = false
            switch result {
            case let .success(read):
                kept(read.id, name: read.onlineID)
            case let .failure(e):
                KyberLog.app.notice("PlayStation : identifiant du compte non lu (\(e.detail, privacy: .public))")
                error = PSText.accountUnread
                byHand = true
            }
        }
    }

    private func pair() {
        guard digits.count == 8, !busy, let found = target.found, let account, let id = store.accountID(account), let pin = UInt32(digits) else { return }
        busy = true
        error = nil
        codeFocused = false
        Task {
            let result = await Task.detached { PlayStation.regist(address: found.address, target: found.target, account: id, pin: pin) }.value
            busy = false
            switch result {
            case let .success(registered):
                let console = store.paired(found, registered: registered, account: account.id)
                app.selected = console.id
                dismiss()
            case let .failure(e):
                error = PSText.regist(e)
            }
        }
    }
}

// MARK: - Connexion Sony

/// La page de connexion de Sony. Son adresse de retour n'est jamais
/// chargée : elle est rendue à `onRedirect`. Rien n'est gardé (ni cookies
/// ni cache) une fois la feuille fermée.
struct PSNWebView: UIViewRepresentable {
    /// La première page est arrivée.
    var onLoaded: () -> Void = {}
    let onRedirect: (URL) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(onLoaded: onLoaded, onRedirect: onRedirect) }

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        // Comme Safari : la page de Sony refuse certains navigateurs intégrés.
        config.applicationNameForUserAgent = "Version/18.0 Mobile/15E148 Safari/604.1"
        let view = WKWebView(frame: .zero, configuration: config)
        view.isOpaque = false
        view.backgroundColor = .clear
        view.navigationDelegate = context.coordinator
        if let url = PlayStation.loginURL { view.load(URLRequest(url: url)) }
        return view
    }

    func updateUIView(_ view: WKWebView, context: Context) {}

    final class Coordinator: NSObject, WKNavigationDelegate {
        let onLoaded: () -> Void
        let onRedirect: (URL) -> Void
        private var done = false

        init(onLoaded: @escaping () -> Void, onRedirect: @escaping (URL) -> Void) {
            self.onLoaded = onLoaded
            self.onRedirect = onRedirect
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            onLoaded()
        }

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
