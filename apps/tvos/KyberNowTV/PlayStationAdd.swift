// Consoles PlayStation dans « Ajouter un ordinateur ou une console » de
// l'Apple TV (textes : docs/link-ui.md, « Consoles PlayStation ») : l'onglet
// « PlayStation » (comptes PlayStation Network, consoles sur ce réseau,
// « Recevoir d'un autre appareil », « Envoyer à un appareil ») et l'écran de
// l'association (choix du compte, compte, code de la console au pavé).
//
// Pas de connexion à Sony sur un téléviseur (pas de navigateur) : un compte
// arrive d'un autre appareil (iCloud, ou l'envoi par le réseau, code à 6
// chiffres), ou son identifiant est saisi à la main.

import KyberKit
import SwiftUI

// MARK: - Onglet « PlayStation »

struct PlayStationPane: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var playstation: PlayStationStore
    @StateObject private var handoff = PSHandoffModel()
    @State private var renaming: PSNAccount?
    @State private var newName = ""
    @State private var removing: PSNAccount?
    /// L'appareil à qui envoyer, le temps de taper son code.
    @State private var sendCode = ""

    var body: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 18) {
                Theme.eyebrow(PSText.tab)
                // Sans compte : recevoir d'abord (le premier écran d'une télé sans compte).
                if playstation.accounts.isEmpty {
                    ReceiveBlock(handoff: handoff)
                }
                Text(PSText.accountsTitle).font(Theme.display(36, 640)).tracking(-0.8).padding(.top, 8)
                if playstation.accounts.isEmpty {
                    Text(PSText.noAccount).font(Theme.sans(21)).foregroundStyle(Theme.muted)
                }
                ForEach(playstation.accounts) { a in
                    accountRow(a)
                }
                HStack(spacing: 16) {
                    Button { app.screen = .consolePair(AppModel.PairRequest(hostID: nil, fromAdd: true)) } label: {
                        HStack(spacing: 12) { Icon("plus", size: 24, bold: true); Text(PSText.addAccount).font(Theme.sans(22, .medium)) }
                            .padding(.horizontal, 24).frame(height: 64)
                    }
                    .buttonStyle(GlassButton(radius: 18, scale: 1.04))
                }

                Text(PSText.consolesTitle).font(Theme.display(36, 640)).tracking(-0.8).padding(.top, 18)
                let list = playstation.onNetwork
                if list.isEmpty {
                    Text(PSText.noConsole).font(Theme.sans(21)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                ForEach(list, id: \.hostID) { f in
                    consoleRow(f)
                }

                if !playstation.accounts.isEmpty {
                    ReceiveBlock(handoff: handoff).padding(.top, 18)
                    SendBlock(handoff: handoff)
                }
            }
            .padding(.bottom, 40)
            // De la marge pour le focus (qui grossit), rendue au-dehors.
            .padding(.horizontal, 20).padding(.top, 12)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, -20).padding(.top, -12)
        .onDisappear { handoff.stop() }
        .alert(PSText.rename, isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } }), presenting: renaming) { a in
            TextField(PSText.nameField, text: $newName)
            Button(PSText.rename) { playstation.rename(a, to: newName); renaming = nil }
            Button(PSText.cancel, role: .cancel) { renaming = nil }
        }
        .alert(removing.map { PSText.removeQuestion(playstation.name($0), consoles: playstation.consoles(of: $0).count) } ?? "",
               isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }), presenting: removing) { a in
            Button(PSText.remove, role: .destructive) { playstation.removeAccount(a); removing = nil }
            Button(PSText.cancel, role: .cancel) { removing = nil }
        } message: { _ in
            Text(PSText.removeHelp)
        }
    }

    private func accountRow(_ a: PSNAccount) -> some View {
        HStack(spacing: 20) {
            Icon("user", size: 30).foregroundStyle(Theme.accent)
            VStack(alignment: .leading, spacing: 6) {
                Text(verbatim: playstation.name(a)).font(Theme.sans(25, .semibold)).lineLimit(1)
                Text(PSText.consoleCount(playstation.consoles(of: a).count)).font(Theme.sans(18)).foregroundStyle(Theme.muted)
            }
            Spacer()
            Button(PSText.rename) { newName = playstation.name(a); renaming = a }
                .buttonStyle(ChoiceButton(selected: false, height: 56, light: true))
            Button(PSText.remove) { removing = a }
                .buttonStyle(ChoiceButton(selected: false, height: 56, light: true))
        }
        .padding(.horizontal, 24)
        .frame(height: 92)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
        .focusSection()
    }

    /// « {nom} · PS5 », son état ; pas associée : « Associer » ; associée :
    /// « Associée : {pseudos} » et, s'il reste un compte, « Associer avec un autre compte ».
    private func consoleRow(_ f: PlayStation.Found) -> some View {
        let kept = playstation.console(hostID: f.hostID)
        let accounts = kept.map { playstation.pairedAccounts($0) } ?? []
        let state = f.ready ? PSText.on : PSText.standby
        return HStack(spacing: 20) {
            Icon("game-controller", size: 30).foregroundStyle(Theme.accent)
            VStack(alignment: .leading, spacing: 6) {
                Text(verbatim: "\(f.name) · \(f.model)").font(Theme.sans(25, .semibold)).lineLimit(1)
                Text(verbatim: accounts.isEmpty ? "\(state) · \(f.address)" : "\(state) · \(PSText.pairedWith(accounts.map { playstation.name($0) }))")
                    .font(Theme.sans(18)).foregroundStyle(Theme.muted).lineLimit(1)
            }
            Spacer()
            if accounts.isEmpty {
                Button(PSText.pair) { app.screen = .consolePair(AppModel.PairRequest(hostID: f.hostID, fromAdd: true)) }
                    .buttonStyle(PrimaryButton(height: 56, fontSize: 21))
            } else if !playstation.unpairedAccounts(hostID: f.hostID).isEmpty {
                Button(PSText.pairAnother) { app.screen = .consolePair(AppModel.PairRequest(hostID: f.hostID, fromAdd: true)) }
                    .buttonStyle(ChoiceButton(selected: false, height: 56, light: true))
            }
        }
        .padding(.horizontal, 24)
        .frame(height: 92)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(accounts.isEmpty ? Theme.accentFill.opacity(0.12) : Color.white.opacity(0.04)))
        .focusSection()
    }
}

// MARK: - Recevoir d'un autre appareil

/// Le code à 6 chiffres en grand, l'aide, puis « {n} compte(s) et {n}
/// console(s) reçus. » ; après 5 min, « Code expiré. » et « Nouveau code ».
struct ReceiveBlock: View {
    @ObservedObject var handoff: PSHandoffModel
    @EnvironmentObject private var playstation: PlayStationStore

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(spacing: 14) {
                Icon("device-mobile", size: 30).foregroundStyle(Theme.accent)
                Text(PSText.receiveTitle).font(Theme.display(36, 640)).tracking(-0.8)
            }
            if let code = handoff.code, !handoff.expired {
                Text(verbatim: PSHandoffModel.spaced(code))
                    .font(Theme.mono(84, .medium)).tracking(8)
                    .foregroundStyle(Theme.neonCore)
                    .accessibilityLabel(Text(verbatim: code.map(String.init).joined(separator: " ")))
                Text(PSText.receiveHelp).font(Theme.sans(21)).foregroundStyle(Theme.text2).lineSpacing(6)
                    .fixedSize(horizontal: false, vertical: true)
            }
            if let received = handoff.received {
                HStack(spacing: 12) {
                    Icon("check", size: 26, bold: true).foregroundStyle(Theme.ok)
                    Text(received).font(Theme.sans(23, .semibold))
                }
            }
            if handoff.expired {
                Text(PSText.codeExpired).font(Theme.sans(22)).foregroundStyle(Theme.warnText)
                Button(PSText.newCode) { handoff.startReceiving(store: playstation) }
                    .buttonStyle(PrimaryButton(height: 64, fontSize: 22))
            }
        }
        .padding(28)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 22, style: .continuous).fill(Color.white.opacity(0.05)))
        .onAppear { if handoff.code == nil, !handoff.expired { handoff.startReceiving(store: playstation) } }
    }
}

// MARK: - Envoyer à un appareil

/// Les appareils du réseau qui attendent un envoi ; un clic ouvre la saisie
/// du code affiché sur cet appareil (pavé), puis « Envoyer ».
struct SendBlock: View {
    @ObservedObject var handoff: PSHandoffModel
    @EnvironmentObject private var playstation: PlayStationStore
    @State private var code = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(PSText.sendTitle).font(Theme.display(36, 640)).tracking(-0.8)
            if let to = handoff.chosen {
                Text(PSText.enterCodeOn(to.name)).font(Theme.sans(21)).foregroundStyle(Theme.text2)
                DigitPad(code: $code, length: 6, disabled: handoff.sending)
                HStack(spacing: 18) {
                    Button(PSText.send) { handoff.send(code: code, store: playstation) }
                        .buttonStyle(PrimaryButton(height: 64, fontSize: 22))
                        .disabled(code.count != 6 || handoff.sending)
                    Button(PSText.cancel) { handoff.chosen = nil; code = "" }
                        .buttonStyle(ChoiceButton(selected: false, height: 64, light: true))
                }
            } else {
                if handoff.waiting.isEmpty {
                    Text(PSText.noDeviceWaiting).font(Theme.sans(21)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                ForEach(handoff.waiting, id: \.address) { w in
                    Button { code = ""; handoff.choose(w) } label: {
                        HStack(spacing: 16) {
                            Icon("device-mobile", size: 26).foregroundStyle(Theme.accent)
                            Text(verbatim: "\(w.name) · \(PSPlatform.label(w.platform))").font(Theme.sans(23, .semibold))
                            Spacer()
                            Icon("arrow-right", size: 22, bold: true).foregroundStyle(Theme.faint)
                        }
                        .padding(.horizontal, 24).frame(height: 76)
                    }
                    .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
                }
            }
            if let result = handoff.sendResult {
                Text(result.text).font(Theme.sans(21)).foregroundStyle(result.ok ? Theme.text : Theme.warnText)
            }
        }
        .padding(28)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 22, style: .continuous).fill(Color.white.opacity(0.05)))
        .onAppear { handoff.startDiscovery() }
    }
}

// MARK: - Pavé de chiffres

/// Des cases et un pavé à la télécommande (1 à 9, effacer, 0), comme
/// « Rejoindre avec un code » ; un chiffre de trop est ignoré.
struct DigitPad: View {
    @Binding var code: String
    let length: Int
    var disabled = false
    /// Le focus va au « 1 » à l'apparition.
    var autoFocus = true
    @FocusState private var focus: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(spacing: 10) {
                ForEach(0..<length, id: \.self) { i in
                    if i > 0, i % (length == 8 ? 4 : 3) == 0 { Spacer().frame(width: 16) }
                    let chars = Array(code)
                    Text(verbatim: i < chars.count ? String(chars[i]) : "")
                        .font(Theme.mono(50, .medium))
                        .foregroundStyle(Theme.neonCore)
                        .frame(width: 66, height: 92)
                        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Color.white.opacity(0.06)))
                        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous)
                            .strokeBorder(i == code.count ? Theme.accent : Color.white.opacity(0.12), lineWidth: i == code.count ? 2.5 : 1))
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text(verbatim: code.map(String.init).joined(separator: " ")))
            VStack(spacing: 12) {
                ForEach(0..<3, id: \.self) { row in
                    HStack(spacing: 12) {
                        ForEach(1...3, id: \.self) { col in key(row * 3 + col) }
                    }
                }
                HStack(spacing: 12) {
                    Button { if !code.isEmpty { code.removeLast() } } label: {
                        Image(systemName: "delete.left").font(.system(size: 30, weight: .medium)).frame(width: 120, height: 72)
                    }
                    .buttonStyle(GlassButton(radius: 18, scale: 1.08))
                    .focused($focus, equals: -1)
                    .accessibilityLabel(Text("Effacer"))
                    key(0)
                    Spacer().frame(width: 120)
                }
            }
            .disabled(disabled)
            .focusSection()
        }
        .onAppear {
            guard autoFocus else { return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { focus = 1 }
        }
    }

    private func key(_ n: Int) -> some View {
        Button { if code.count < length { code += String(n) } } label: {
            Text(verbatim: "\(n)").font(Theme.mono(36, .medium)).frame(width: 120, height: 72)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.08))
        .focused($focus, equals: n)
    }
}

// MARK: - Association

/// L'association d'une console, ou l'ajout d'un compte (sans console) :
/// choix du compte (deux comptes ou plus pas encore associés), compte
/// (recevoir d'un autre appareil, ou l'identifiant à la main), puis le code
/// à 8 chiffres affiché sur la console.
struct ConsolePairView: View {
    let request: AppModel.PairRequest
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var playstation: PlayStationStore
    @StateObject private var handoff = PSHandoffModel()

    private enum Step { case choose, account, code }
    @State private var step: Step = .account
    @State private var byHand = false
    @State private var manualID = ""
    @State private var manualName = ""
    @State private var account: PSNAccount?
    @State private var already = false
    @State private var code = ""
    @State private var busy = false
    @State private var error: String?
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, account(UUID), add, byHand, id, name, proceed, pair }

    /// La console à associer : telle qu'elle répond, sinon telle qu'elle est gardée.
    private var found: PlayStation.Found? {
        guard let id = request.hostID else { return nil }
        if let f = playstation.onNetwork.first(where: { $0.hostID == id }) { return f }
        if let c = playstation.console(hostID: id) { return PairTarget(console: c, live: playstation.live(c)).found }
        return nil
    }
    private var free: [PSNAccount] { request.hostID.map { playstation.unpairedAccounts(hostID: $0) } ?? [] }
    /// Identifiant saisi à la main : 8 octets en base64 (12 caractères).
    private var manualValid: Bool { Data(base64Encoded: manualID.trimmingCharacters(in: .whitespaces))?.count == 8 }

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

            VStack(alignment: .leading, spacing: 14) {
                if let found {
                    Text(verbatim: found.name).font(Theme.display(64, 660)).tracking(-64 * 0.035).lineLimit(2).minimumScaleFactor(0.6)
                    Text(verbatim: found.model).font(Theme.sans(26)).foregroundStyle(Theme.muted)
                } else {
                    Text(PSText.addAccount).font(Theme.display(64, 660)).tracking(-64 * 0.035)
                }
                if let account, step == .code {
                    HStack(spacing: 12) {
                        Icon("user", size: 26, bold: true).foregroundStyle(Theme.accent)
                        Text(verbatim: PSText.accountLine(playstation.name(account))).font(Theme.sans(24, .semibold))
                    }
                    .padding(.top, 20)
                }
                if already, step == .code {
                    Text(PSText.alreadyHere).font(Theme.sans(21)).foregroundStyle(Theme.muted)
                }
            }
            .frame(width: 680, alignment: .leading)
            .padding(.leading, 96).padding(.top, 180)

            Group {
                switch step {
                case .choose: chooseStep
                case .account: accountStep
                case .code: codeStep
                }
            }
            .padding(.horizontal, 60).padding(.vertical, 56)
            .frame(width: 1920 - 840 - 96, height: 1080 - 180 - 96, alignment: .topLeading)
            .glass(30)
            .focusSection()
            .padding(.leading, 840).padding(.top, 180)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .onExitCommand { leave() }
        .defaultFocus($focus, step == .choose ? free.first.map { .account($0.id) } : step == .account ? .byHand : nil)
        .onAppear {
            #if DEBUG
            if UserDefaults.standard.bool(forKey: "pairAccountStep") {
                step = .account
                byHand = UserDefaults.standard.bool(forKey: "pairByHand")
                return
            }
            if UserDefaults.standard.string(forKey: "screen") == "consolePair", let first = playstation.accounts.first {
                account = first
                step = .code
                return
            }
            #endif
            start()
        }
        .onDisappear { handoff.stop() }
        // Des comptes arrivent d'un autre appareil pendant l'étape Compte : la suite.
        .onChange(of: playstation.accounts.count) { _, _ in
            guard step == .account, !byHand, handoff.received != nil else { return }
            if request.hostID == nil { leave(); return }
            start()
        }
    }

    /// Deux comptes ou plus pas encore associés : le choix ; un seul : le code ; aucun : l'étape Compte.
    private func start() {
        switch request.hostID == nil ? 0 : free.count {
        case 0: step = .account
        case 1: account = free[0]; step = .code
        default: step = .choose
        }
    }

    private func leave() {
        app.screen = request.fromAdd ? .addHost : .home
    }

    // « Avec quel compte ? »
    private var chooseStep: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text(PSText.whichAccount).font(Theme.display(44, 640)).tracking(-1)
            ForEach(free) { a in
                Button {
                    account = a
                    error = nil
                    step = .code
                } label: {
                    HStack(spacing: 18) {
                        Icon("user", size: 28).foregroundStyle(Theme.accent)
                        VStack(alignment: .leading, spacing: 4) {
                            Text(verbatim: playstation.name(a)).font(Theme.sans(25, .semibold))
                            Text(PSText.consoleCount(playstation.consoles(of: a).count)).font(Theme.sans(18)).foregroundStyle(Theme.muted)
                        }
                        Spacer()
                        Icon("arrow-right", size: 22, bold: true).foregroundStyle(Theme.faint)
                    }
                    .padding(.horizontal, 26).frame(height: 92)
                }
                .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
                .focused($focus, equals: .account(a.id))
            }
            Button { error = nil; step = .account } label: {
                HStack(spacing: 12) { Icon("plus", size: 24, bold: true); Text(PSText.addAccount).font(Theme.sans(22, .medium)) }
                    .padding(.horizontal, 24).frame(height: 64)
            }
            .buttonStyle(GlassButton(radius: 18, scale: 1.04))
            .focused($focus, equals: .add)
            Spacer(minLength: 0)
        }
    }

    // Compte : recevoir d'un autre appareil, ou saisir l'identifiant à la main.
    private var accountStep: some View {
        ScrollView(.vertical, showsIndicators: false) {
            VStack(alignment: .leading, spacing: 22) {
                if byHand {
                    VStack(alignment: .leading, spacing: 10) {
                        Text(PSText.accountField).font(Theme.sans(19)).foregroundStyle(Theme.muted)
                        TextField(String("AbCdEfGhIjk="), text: $manualID)
                            .font(Theme.mono(28))
                            .autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                            .focused($focus, equals: .id)
                            .frame(width: 700)
                    }
                    VStack(alignment: .leading, spacing: 10) {
                        Text(PSText.nameField).font(Theme.sans(19)).foregroundStyle(Theme.muted)
                        TextField(PSText.nameField, text: $manualName)
                            .font(Theme.sans(28))
                            .autocorrectionDisabled()
                            .focused($focus, equals: .name)
                            .frame(width: 700)
                        Text(PSText.nameHelp).font(Theme.sans(18)).foregroundStyle(Theme.faint)
                    }
                    if let error { Text(error).font(Theme.sans(21)).foregroundStyle(Theme.warnText) }
                    HStack(spacing: 18) {
                        Button(String(localized: "Continuer")) { useManualID() }
                            .buttonStyle(PrimaryButton(height: 72, fontSize: 25))
                            .disabled(!manualValid)
                            .focused($focus, equals: .proceed)
                        Button(PSText.receiveTitle) { byHand = false }
                            .buttonStyle(ChoiceButton(selected: false, height: 72, light: true))
                    }
                } else {
                    ReceiveBlock(handoff: handoff)
                    Button { byHand = true; error = nil } label: {
                        HStack(spacing: 12) { Icon("keyboard", size: 26); Text(PSText.byHand).font(Theme.sans(22, .medium)) }
                            .padding(.horizontal, 24).frame(height: 64)
                    }
                    .buttonStyle(GlassButton(radius: 18, scale: 1.04))
                    .focused($focus, equals: .byHand)
                }
            }
            .padding(.horizontal, 20).padding(.vertical, 12)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, -20).padding(.vertical, -12)
    }

    // Le code de la console.
    private var codeStep: some View {
        VStack(alignment: .leading, spacing: 22) {
            Text(PSText.codeHelp).font(Theme.sans(22)).foregroundStyle(Theme.text2).lineSpacing(6)
                .fixedSize(horizontal: false, vertical: true)
            DigitPad(code: $code, length: 8, disabled: busy)
                .accessibilityLabel(PSText.codeField)
            if let error { Text(error).font(Theme.sans(21)).foregroundStyle(Theme.warnText) }
            Spacer(minLength: 0)
            HStack(spacing: 18) {
                Button(action: pair) {
                    HStack(spacing: 12) {
                        if busy { ProgressView() }
                        Text(PSText.pair)
                    }
                }
                .buttonStyle(PrimaryButton(height: 72, fontSize: 25))
                .disabled(code.count != 8 || busy)
                .focused($focus, equals: .pair)
                Text(PSText.codeField).font(Theme.sans(19)).foregroundStyle(Theme.muted)
            }
        }
        .onChange(of: code) { _, new in if new.count == 8 { focus = .pair } }
    }

    private func useManualID() {
        guard manualValid else { return }
        let (a, existed) = playstation.keep(accountID: manualID.trimmingCharacters(in: .whitespaces), name: manualName)
        guard request.hostID != nil else {
            leave()
            return
        }
        account = a
        already = existed
        error = nil
        step = .code
    }

    private func pair() {
        guard code.count == 8, !busy, let found, let account, let id = playstation.accountID(account), let pin = UInt32(code) else { return }
        busy = true
        error = nil
        Task {
            let result = await Task.detached { PlayStation.regist(address: found.address, target: found.target, account: id, pin: pin) }.value
            busy = false
            switch result {
            case let .success(registered):
                let console = playstation.paired(found, registered: registered, account: account.id)
                app.selected = console.id
                app.screen = .home
            case let .failure(e):
                error = PSText.regist(e)
            }
        }
    }
}
